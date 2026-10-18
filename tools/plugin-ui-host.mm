// Local, silent AU editor harness. It never opens or changes a DAW project.
#import <Cocoa/Cocoa.h>
#import <AudioUnit/AudioUnit.h>
#import <AudioUnit/AUCocoaUIView.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstring>

static double hostBeat=0.;static unsigned hostClockCalls=0;
static OSStatus beatTempo(void*,Float64* beat,Float64* bpm) { hostClockCalls++;if(beat)*beat=hostBeat;if(bpm)*bpm=128.;return noErr; }
static OSStatus transport(void*,Boolean* playing,Boolean* changed,Float64* sample,Boolean* cycle,Float64* start,Float64* end) {
  if(playing)*playing=true;if(changed)*changed=false;if(sample)*sample=hostBeat*48000.*60./128.;
  if(cycle)*cycle=false;if(start)*start=0.;if(end)*end=4.;return noErr;
}
static OSStatus silence(void*,AudioUnitRenderActionFlags*,const AudioTimeStamp*,UInt32,UInt32 frames,AudioBufferList* data) {
  for(UInt32 c=0;c<data->mNumberBuffers;c++)std::memset(data->mBuffers[c].mData,0,frames*sizeof(float));return noErr;
}
#define CHECK(x) do { if(!(x)) { NSLog(@"AU editor harness failed line %d",__LINE__);std::exit(1); } } while(0)
@interface EditorHost : NSObject<NSApplicationDelegate,NSWindowDelegate>
@property NSWindow* window;
@property AudioUnit unit;
@property id factory;
@property NSTimer* timer;
@end
@implementation EditorHost
- (void)applicationDidFinishLaunching:(NSNotification*)notification {
  NSString* path=[[NSBundle mainBundle] objectForInfoDictionaryKey:@"PluginPath"];
  NSBundle* bundle=[NSBundle bundleWithPath:path];CHECK([bundle load]);
  auto url=CFURLCreateFromFileSystemRepresentation(nullptr,reinterpret_cast<const UInt8*>(path.UTF8String),std::strlen(path.UTF8String),true);
  auto cfBundle=CFBundleCreate(nullptr,url);CHECK(cfBundle);
  auto entry=reinterpret_cast<AudioComponentFactoryFunction>(CFBundleGetFunctionPointerForName(cfBundle,CFSTR("KekLocator_Factory")));CHECK(entry);
  AudioComponentDescription desc{kAudioUnitType_MusicEffect,'EcR1','ErRs',0,0};
  auto component=AudioComponentRegister(&desc,CFSTR("Erika Rust: KekLocator"),0x10000,entry);CHECK(component);
  AudioUnit unit=nullptr;CHECK(AudioComponentInstanceNew(component,&unit)==noErr);self.unit=unit;
  AudioStreamBasicDescription format{48000.,kAudioFormatLinearPCM,kAudioFormatFlagIsFloat|kAudioFormatFlagIsNonInterleaved|kAudioFormatFlagsNativeEndian,4,1,4,2,32,0};
  CHECK(AudioUnitSetProperty(unit,kAudioUnitProperty_StreamFormat,kAudioUnitScope_Input,0,&format,sizeof(format))==noErr);
  CHECK(AudioUnitSetProperty(unit,kAudioUnitProperty_StreamFormat,kAudioUnitScope_Output,0,&format,sizeof(format))==noErr);
  HostCallbackInfo callbacks{};callbacks.hostUserData=&hostBeat;callbacks.beatAndTempoProc=beatTempo;callbacks.transportStateProc=transport;
  CHECK(AudioUnitSetProperty(unit,kAudioUnitProperty_HostCallbacks,kAudioUnitScope_Global,0,&callbacks,sizeof(callbacks))==noErr);
  AURenderCallbackStruct input{silence,nullptr};CHECK(AudioUnitSetProperty(unit,kAudioUnitProperty_SetRenderCallback,kAudioUnitScope_Input,0,&input,sizeof(input))==noErr);
  CHECK(AudioUnitInitialize(unit)==noErr);CHECK(AudioUnitSetParameter(unit,24,kAudioUnitScope_Global,0,1.,0)==noErr);
  AudioUnitCocoaViewInfo info{};UInt32 size=sizeof(info);
  CHECK(AudioUnitGetProperty(unit,kAudioUnitProperty_CocoaUI,kAudioUnitScope_Global,0,&info,&size)==noErr);
  NSBundle* viewBundle=[NSBundle bundleWithURL:(__bridge NSURL*)info.mCocoaAUViewBundleLocation];CHECK([viewBundle load]);
  Class viewClass=[viewBundle classNamed:(__bridge NSString*)info.mCocoaAUViewClass[0]];CHECK(viewClass);
  id<AUCocoaUIBase> factory=[[viewClass alloc] init];self.factory=factory;
  NSView* view=[factory uiViewForAudioUnit:unit withSize:NSMakeSize(1060,790)];CHECK(view);
  self.window=[[NSWindow alloc] initWithContentRect:NSMakeRect(80,80,1060,790) styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
  self.window.title=@"KekLocator — native AU verification";self.window.delegate=self;
  view.frame=self.window.contentView.bounds;view.autoresizingMask=NSViewWidthSizable|NSViewHeightSizable;
  [self.window.contentView addSubview:view];[self.window makeKeyAndOrderFront:nil];
  self.timer=[NSTimer timerWithTimeInterval:512./48000. repeats:YES block:^(NSTimer*) {
    static std::array<float,512> left{},right{};static int64_t stamp=0;
    struct Stereo { UInt32 count;AudioBuffer buffers[2]; } audio{2,{{1,2048,left.data()},{1,2048,right.data()}}};
    AudioTimeStamp time{};time.mFlags=kAudioTimeStampSampleTimeValid;time.mSampleTime=stamp;AudioUnitRenderActionFlags flags=0;
    CHECK(AudioUnitRender(unit,&flags,&time,0,512,reinterpret_cast<AudioBufferList*>(&audio))==noErr);
    stamp+=512;hostBeat+=512./48000.*128./60.;
    if(stamp%51200==0) {
      auto report=[[[NSBundle mainBundle] objectForInfoDictionaryKey:@"ReportPath"] UTF8String];
      UInt32 bypass=0,propertySize=sizeof(bypass);AudioUnitGetProperty(unit,kAudioUnitProperty_BypassEffect,kAudioUnitScope_Global,0,&bypass,&propertySize);
      float peak=0;for(float sample:left)peak=std::max(peak,std::abs(sample));
      if(FILE* f=std::fopen(report,"w")){std::fprintf(f,"rendered_frames=%lld beat=%.6f host_bpm=128 callback_calls=%u bypass=%u peak=%f\n",stamp,hostBeat,hostClockCalls,bypass,peak);std::fclose(f);}
    }
  }];
  [[NSRunLoop mainRunLoop] addTimer:self.timer forMode:NSRunLoopCommonModes];
  CFRelease(info.mCocoaAUViewBundleLocation);CFRelease(info.mCocoaAUViewClass[0]);CFRelease(cfBundle);CFRelease(url);
}
- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication*)sender { return YES; }
- (void)applicationWillTerminate:(NSNotification*)notification { [self.timer invalidate];if(self.unit){AudioUnitUninitialize(self.unit);AudioComponentInstanceDispose(self.unit);} }
@end
int main() { @autoreleasepool { NSApplication* app=[NSApplication sharedApplication];[app setActivationPolicy:NSApplicationActivationPolicyRegular];EditorHost* delegate=[EditorHost new];app.delegate=delegate;[app run]; } }
