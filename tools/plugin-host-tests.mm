// Real plug-in ABI tests. No DAW/project is opened or modified.
#ifdef _WIN32
#define NOMINMAX
#include <windows.h>
#else
#include <AudioToolbox/AudioToolbox.h>
#include <CoreFoundation/CoreFoundation.h>
#include <dlfcn.h>
#endif
#include <clap/clap.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
#include "../plugins/engine.h"

#define REQUIRE(x) do{if(!(x)){std::fprintf(stderr,"FAIL line %d: %s\n",__LINE__,#x);std::exit(1);}}while(0)
#include "timed-reference.h"
#ifndef _WIN32
struct Transport{double bpm=120.,beat=0.,sample=0.;bool playing=true;};
static OSStatus beatTempo(void* p,Float64* beat,Float64* bpm){auto&t=*static_cast<Transport*>(p);if(beat)*beat=t.beat;if(bpm)*bpm=t.bpm;return noErr;}
static OSStatus transport(void* p,Boolean* play,Boolean* changed,Float64* sample,Boolean* cycle,Float64* start,Float64* end){auto&t=*static_cast<Transport*>(p);if(play)*play=t.playing;if(changed)*changed=false;if(sample)*sample=t.sample;if(cycle)*cycle=false;if(start)*start=0.;if(end)*end=4.;return noErr;}
static OSStatus input(void*,AudioUnitRenderActionFlags*,const AudioTimeStamp*,UInt32,UInt32 frames,AudioBufferList* b){for(UInt32 c=0;c<b->mNumberBuffers;c++)std::memset(b->mBuffers[c].mData,0,frames*sizeof(float));return noErr;}
#endif
static void compare(const float* l,const float* r,std::vector<float>&expected,int frames,double&peak){
  for(int i=0;i<frames;i++)for(int c=0;c<2;c++){
    float actual=c?r[i]:l[i],want=expected[i*2+c];REQUIRE(std::isfinite(actual));
    if(std::abs(actual-want)>0.000002f){std::fprintf(stderr,"sample %d/%d actual %.9g reference %.9g\n",i,c,actual,want);REQUIRE(false);}
    peak=std::max(peak,std::abs(double(actual)));
  }
}
#ifndef _WIN32
static void au(const char* path){
  CFURLRef url=CFURLCreateFromFileSystemRepresentation(nullptr,reinterpret_cast<const UInt8*>(path),std::strlen(path),true);
  CFBundleRef bundle=CFBundleCreate(nullptr,url);REQUIRE(bundle);REQUIRE(CFBundleLoadExecutable(bundle));
  auto factory=reinterpret_cast<AudioComponentFactoryFunction>(CFBundleGetFunctionPointerForName(bundle,CFSTR("KekLocator_Factory")));REQUIRE(factory);
  AudioComponentDescription desc{kAudioUnitType_MusicEffect,'EcR1','ErRs',0,0};
  auto component=AudioComponentRegister(&desc,CFSTR("Erika Rust: KekLocator"),0x10000,factory);REQUIRE(component);
  for(int mode:{0,1})for(double rate:{44100.,48000.,96000.,192000.}){
    AudioUnit unit=nullptr;REQUIRE(AudioComponentInstanceNew(component,&unit)==noErr);
    AudioStreamBasicDescription f{rate,kAudioFormatLinearPCM,kAudioFormatFlagIsFloat|kAudioFormatFlagIsNonInterleaved|kAudioFormatFlagsNativeEndian,4,1,4,2,32,0};
    REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_StreamFormat,kAudioUnitScope_Input,0,&f,sizeof(f))==noErr);
    REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_StreamFormat,kAudioUnitScope_Output,0,&f,sizeof(f))==noErr);
    // A valid AU host can use the default buffer limit without setting it.
    if(rate!=48000.){UInt32 max=1024;REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_MaximumFramesPerSlice,kAudioUnitScope_Global,0,&max,sizeof(max))==noErr);}
    Transport t;HostCallbackInfo callbacks{};callbacks.hostUserData=&t;callbacks.beatAndTempoProc=beatTempo;callbacks.transportStateProc=transport;
    REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_HostCallbacks,kAudioUnitScope_Global,0,&callbacks,sizeof(callbacks))==noErr);
    AURenderCallbackStruct renderInput{input,nullptr};REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_SetRenderCallback,kAudioUnitScope_Input,0,&renderInput,sizeof(renderInput))==noErr);
    REQUIRE(AudioUnitInitialize(unit)==noErr);
    UInt32 maximumFrames=0,maximumSize=sizeof(maximumFrames);
    REQUIRE(AudioUnitGetProperty(unit,kAudioUnitProperty_MaximumFramesPerSlice,kAudioUnitScope_Global,0,&maximumFrames,&maximumSize)==noErr);
    REQUIRE(maximumFrames>0);
    REQUIRE(AudioUnitSetParameter(unit,24,kAudioUnitScope_Global,0,1.,0)==noErr);
    REQUIRE(AudioUnitSetParameter(unit,18,kAudioUnitScope_Global,0,0.,0)==noErr);
    REQUIRE(AudioUnitSetParameter(unit,21,kAudioUnitScope_Global,0,0.,0)==noErr);
    REQUIRE(AudioUnitSetParameter(unit,64,kAudioUnitScope_Global,0,mode,0)==noErr);
    if(mode){for(auto [id,v]:std::vector<std::pair<int,float>>{{65,.8f},{69,.7f},{71,1.f},{72,.65f},{88,.6f},{104,3.f/7.f},{120,4.f/31.f},{138,.3f}})REQUIRE(AudioUnitSetParameter(unit,id,kAudioUnitScope_Global,0,v,0)==noErr);}
    void* reference=ec_daw_create(rate);REQUIRE(reference);
    for(int p=0;p<EC_PARAM_COUNT;p++){AudioUnitParameterValue v;REQUIRE(AudioUnitGetParameter(unit,p,kAudioUnitScope_Global,0,&v)==noErr);REQUIRE(ec_daw_set(reference,p,v)==0);}
    Float64 latency=0.;UInt32 size=sizeof(latency);REQUIRE(AudioUnitGetProperty(unit,kAudioUnitProperty_Latency,kAudioUnitScope_Global,0,&latency,&size)==noErr);
    REQUIRE(std::abs(latency*rate-ec_daw_latency(reference))<0.01);
    std::array<float,1024> l{},r{};struct Stereo{UInt32 count;AudioBuffer b[2];}data{2,{{1,4096,l.data()},{1,4096,r.data()}}};
    std::vector<float> expected(2048);double peak=0.;int stamp=0;
    for(int phase=0;phase<6;phase++){
      t.bpm=phase==2?160.:phase==3?90.:120.;t.beat=phase==3?0.125:phase*.75;t.playing=phase!=4;
      for(int requested:{1,17,128,511,1024}){
        int n=std::min(requested,static_cast<int>(maximumFrames));
        std::fill(expected.begin(),expected.end(),0.f);REQUIRE(ec_daw_process(reference,expected.data(),n,t.bpm,t.beat,t.playing)==0);
        AudioTimeStamp time{};time.mFlags=kAudioTimeStampSampleTimeValid;time.mSampleTime=stamp;AudioUnitRenderActionFlags flags=0;
        data.b[0].mDataByteSize=data.b[1].mDataByteSize=n*4;
        REQUIRE(AudioUnitRender(unit,&flags,&time,0,n,reinterpret_cast<AudioBufferList*>(&data))==noErr);
        compare(l.data(),r.data(),expected,n,peak);stamp+=n;t.sample+=n;t.beat+=n/rate*t.bpm/60.;
      }
    }
    {const int n=std::min(511,static_cast<int>(maximumFrames));t.bpm=120.;t.beat=4.;t.playing=true;
      std::vector<EcPoint> points{{n/8,23,.2f},{n/3,23,.75f},{n*2/3,23,.4f},{n*4/5,18,ec_daw_cc_value(96)}};
      for(size_t i=0;i<3;i++)REQUIRE(AudioUnitSetParameter(unit,points[i].id,kAudioUnitScope_Global,0,points[i].value,points[i].offset)==noErr);
      REQUIRE(MusicDeviceMIDIEvent(unit,0xb0,77,96,points[3].offset)==noErr);
      timed_reference(reference,expected,n,rate,t.bpm,t.beat,points);
      AudioTimeStamp time{};time.mFlags=kAudioTimeStampSampleTimeValid;time.mSampleTime=stamp;AudioUnitRenderActionFlags flags=0;data.b[0].mDataByteSize=data.b[1].mDataByteSize=n*4;
      REQUIRE(AudioUnitRender(unit,&flags,&time,0,n,reinterpret_cast<AudioBufferList*>(&data))==noErr);compare(l.data(),r.data(),expected,n,peak);stamp+=n;t.beat+=n/rate*2.;
      // AU linear ramp crosses a block boundary; each sample has a known value.
      AudioUnitParameterEvent event{};event.scope=kAudioUnitScope_Global;event.parameter=23;event.eventType=kParameterEvent_Ramped;event.eventValues.ramp.startBufferOffset=3;event.eventValues.ramp.durationInFrames=700;event.eventValues.ramp.startValue=.4;event.eventValues.ramp.endValue=.9;
      REQUIRE(AudioUnitScheduleParameters(unit,&event,1)==noErr);
      for(int block=0;block<2;block++){
        std::fill(expected.begin(),expected.end(),0.f);for(int i=0;i<n;i++){int elapsed=block*n+i-3;if(elapsed>=0)REQUIRE(ec_daw_set(reference,23,.4f+(.9f-.4f)*std::clamp(elapsed/700.f,0.f,1.f))==0);REQUIRE(ec_daw_process(reference,expected.data()+i*2,1,120.,t.beat+i/rate*2.,1)==0);}
        time.mSampleTime=stamp;data.b[0].mDataByteSize=data.b[1].mDataByteSize=n*4;flags=0;REQUIRE(AudioUnitRender(unit,&flags,&time,0,n,reinterpret_cast<AudioBufferList*>(&data))==noErr);compare(l.data(),r.data(),expected,n,peak);stamp+=n;t.beat+=n/rate*2.;
      }
    }
    // A ramp begun during preroll is already partway through at frame zero.
    {const int n=std::min(257,static_cast<int>(maximumFrames));
      AudioUnitParameterEvent event{};event.scope=kAudioUnitScope_Global;event.parameter=23;event.eventType=kParameterEvent_Ramped;event.eventValues.ramp.startBufferOffset=-100;event.eventValues.ramp.durationInFrames=200;event.eventValues.ramp.startValue=.1f;event.eventValues.ramp.endValue=.9f;
      REQUIRE(AudioUnitScheduleParameters(unit,&event,1)==noErr);
      std::fill(expected.begin(),expected.end(),0.f);for(int i=0;i<n;i++){REQUIRE(ec_daw_set(reference,23,.1f+(.9f-.1f)*std::clamp((i+100)/200.f,0.f,1.f))==0);REQUIRE(ec_daw_process(reference,expected.data()+i*2,1,120.,t.beat+i/rate*2.,1)==0);}
      AudioTimeStamp time{};time.mFlags=kAudioTimeStampSampleTimeValid;time.mSampleTime=stamp;AudioUnitRenderActionFlags flags=0;data.b[0].mDataByteSize=data.b[1].mDataByteSize=n*4;REQUIRE(AudioUnitRender(unit,&flags,&time,0,n,reinterpret_cast<AudioBufferList*>(&data))==noErr);compare(l.data(),r.data(),expected,n,peak);
    }
    REQUIRE(peak>0.001);
    CFPropertyListRef saved=nullptr;size=sizeof(saved);REQUIRE(AudioUnitGetProperty(unit,kAudioUnitProperty_ClassInfo,kAudioUnitScope_Global,0,&saved,&size)==noErr);
    REQUIRE(AudioUnitSetParameter(unit,31,kAudioUnitScope_Global,0,.9,0)==noErr);
    REQUIRE(AudioUnitSetProperty(unit,kAudioUnitProperty_ClassInfo,kAudioUnitScope_Global,0,&saved,sizeof(saved))==noErr);
    AudioUnitParameterValue restored;REQUIRE(AudioUnitGetParameter(unit,31,kAudioUnitScope_Global,0,&restored)==noErr);REQUIRE(std::abs(restored-45.f/127.f)<.00001);
    CFRelease(saved);ec_daw_destroy(reference);AudioUnitUninitialize(unit);AudioComponentInstanceDispose(unit);
    std::printf("PASS AU %.0f Hz: transport, tempo, seek/loop, stop, variable blocks, latency, state; mode %d; peak %.6f\n",rate,mode,peak);
  }
  CFRelease(url);CFRelease(bundle);
}
#endif
struct Events{clap_input_events_t api;std::vector<clap_event_param_value_t> data;
 Events(){api.ctx=this;api.size=[](const clap_input_events_t*p)->uint32_t{return static_cast<Events*>(p->ctx)->data.size();};api.get=[](const clap_input_events_t*p,uint32_t i)->const clap_event_header_t*{return &static_cast<Events*>(p->ctx)->data[i].header;};}
 void add(int id,double value,int offset=0){clap_event_param_value_t e{};e.header.size=sizeof(e);e.header.time=offset;e.header.type=CLAP_EVENT_PARAM_VALUE;e.param_id=id;e.note_id=-1;e.port_index=-1;e.channel=-1;e.key=-1;e.value=value;data.push_back(e);}
};
static void clap(const char* path){
#ifdef _WIN32
  auto library=LoadLibraryA(path);REQUIRE(library);
  auto entry=reinterpret_cast<const clap_plugin_entry_t*>(GetProcAddress(library,"clap_entry"));
#else
  void* library=dlopen(path,RTLD_NOW|RTLD_LOCAL);if(!library)std::fprintf(stderr,"%s\n",dlerror());REQUIRE(library);
  auto entry=static_cast<const clap_plugin_entry_t*>(dlsym(library,"clap_entry"));
#endif
  REQUIRE(entry&&entry->init(path));
  auto factory=static_cast<const clap_plugin_factory_t*>(entry->get_factory(CLAP_PLUGIN_FACTORY_ID));REQUIRE(factory&&factory->get_plugin_count(factory)==1);
  const auto descriptor=factory->get_plugin_descriptor(factory,0);
  REQUIRE(descriptor&&std::strcmp(descriptor->name,"KekLocator")==0);
  REQUIRE(std::strcmp(descriptor->id,"local.ErikaRust.Echolocator")==0);
  clap_host_t host{};host.clap_version=CLAP_VERSION;host.name="KekLocator transport tests";host.vendor="Local";host.version="1";host.url="https://github.com/free-audio/clap";
  host.get_extension=[](const clap_host_t*,const char*)->const void*{return nullptr;};host.request_restart=host.request_process=host.request_callback=[](const clap_host_t*){};
  for(int mode:{0,1})for(double rate:{44100.,48000.,96000.,192000.}){
    auto plugin=factory->create_plugin(factory,&host,factory->get_plugin_descriptor(factory,0)->id);REQUIRE(plugin&&plugin->init(plugin));
    auto params=static_cast<const clap_plugin_params_t*>(plugin->get_extension(plugin,CLAP_EXT_PARAMS));REQUIRE(params&&params->count(plugin)==EC_PARAM_COUNT);
    clap_output_events_t outEvents{};outEvents.try_push=[](const clap_output_events_t*,const clap_event_header_t*){return true;};
    Events initial;initial.add(24,1.);initial.add(18,0.);initial.add(21,0.);initial.add(64,mode);
    if(mode){for(auto [id,v]:std::vector<std::pair<int,float>>{{65,.8f},{69,.7f},{71,1.f},{72,.65f},{88,.6f},{104,3.f/7.f},{120,4.f/31.f},{138,.3f}})initial.add(id,v);}params->flush(plugin,&initial.api,&outEvents);
    REQUIRE(plugin->activate(plugin,rate,1,1024));REQUIRE(plugin->start_processing(plugin));
    void* reference=ec_daw_create(rate);for(int id=0;id<EC_PARAM_COUNT;id++){double value=0.;REQUIRE(params->get_value(plugin,id,&value));REQUIRE(ec_daw_set(reference,id,value)==0);}
    auto latency=static_cast<const clap_plugin_latency_t*>(plugin->get_extension(plugin,CLAP_EXT_LATENCY));REQUIRE(latency&&latency->get(plugin)==ec_daw_latency(reference));
    std::array<float,1024> l{},r{},zero{};float* inputs[2]={zero.data(),zero.data()};float* outputs[2]={l.data(),r.data()};
    clap_audio_buffer_t inputBuffer{},outputBuffer{};inputBuffer.data32=inputs;inputBuffer.channel_count=2;outputBuffer.data32=outputs;outputBuffer.channel_count=2;
    Events empty;std::vector<float> expected(2048);double peak=0.;int64_t sample=0;
    for(int phase=0;phase<6;phase++){
      double bpm=phase==2?160.:phase==3?90.:120.,beat=phase==3?.125:phase*.75;bool playing=phase!=4;
      for(int n:{1,17,128,511,1024}){
        clap_event_transport_t transport{};transport.header.size=sizeof(transport);transport.header.type=CLAP_EVENT_TRANSPORT;
        transport.flags=CLAP_TRANSPORT_HAS_TEMPO|CLAP_TRANSPORT_HAS_BEATS_TIMELINE|CLAP_TRANSPORT_HAS_SECONDS_TIMELINE|(playing?CLAP_TRANSPORT_IS_PLAYING:0);
        transport.tempo=bpm;transport.song_pos_beats=std::llround(beat*CLAP_BEATTIME_FACTOR);transport.song_pos_seconds=std::llround(sample/rate*CLAP_SECTIME_FACTOR);
        clap_process_t process{};process.steady_time=sample;process.frames_count=n;process.transport=&transport;process.audio_inputs=&inputBuffer;process.audio_outputs=&outputBuffer;process.audio_inputs_count=process.audio_outputs_count=1;process.in_events=&empty.api;process.out_events=&outEvents;
        std::fill(expected.begin(),expected.end(),0.f);REQUIRE(ec_daw_process(reference,expected.data(),n,bpm,double(transport.song_pos_beats)/CLAP_BEATTIME_FACTOR,playing)==0);
        REQUIRE(plugin->process(plugin,&process)!=CLAP_PROCESS_ERROR);compare(l.data(),r.data(),expected,n,peak);sample+=n;beat+=n/rate*bpm/60.;
      }
    }
    {const int n=511;const double beat=4.;Events scheduled;std::vector<EcPoint> points{{5,23,.2f},{127,23,.75f},{300,23,.4f}};for(const auto&p:points)scheduled.add(p.id,p.value,p.offset);
      clap_event_transport_t transport{};transport.header.size=sizeof(transport);transport.header.type=CLAP_EVENT_TRANSPORT;transport.flags=CLAP_TRANSPORT_HAS_TEMPO|CLAP_TRANSPORT_HAS_BEATS_TIMELINE|CLAP_TRANSPORT_IS_PLAYING;transport.tempo=120.;transport.song_pos_beats=std::llround(beat*CLAP_BEATTIME_FACTOR);
      clap_process_t process{};process.frames_count=n;process.transport=&transport;process.audio_inputs=&inputBuffer;process.audio_outputs=&outputBuffer;process.audio_inputs_count=process.audio_outputs_count=1;process.in_events=&scheduled.api;process.out_events=&outEvents;
      timed_reference(reference,expected,n,rate,120.,beat,points);REQUIRE(plugin->process(plugin,&process)!=CLAP_PROCESS_ERROR);compare(l.data(),r.data(),expected,n,peak);
      char display[128];REQUIRE(params->value_to_text(plugin,22,.6,display,sizeof(display)));REQUIRE(std::strstr(display,"-6.0 dB"));double value;REQUIRE(params->text_to_value(plugin,22,"-6 dB",&value));REQUIRE(std::abs(value-.6)<.00001);
    }
    REQUIRE(peak>0.001);plugin->stop_processing(plugin);plugin->deactivate(plugin);
    auto state=static_cast<const clap_plugin_state_t*>(plugin->get_extension(plugin,CLAP_EXT_STATE));REQUIRE(state);
    std::vector<uint8_t> bytes;clap_ostream_t output{};output.ctx=&bytes;output.write=[](const clap_ostream_t*p,const void*b,uint64_t size)->int64_t{auto&v=*static_cast<std::vector<uint8_t>*>(p->ctx);v.insert(v.end(),static_cast<const uint8_t*>(b),static_cast<const uint8_t*>(b)+size);return size;};REQUIRE(state->save(plugin,&output));
    Events change;change.add(31,.9);params->flush(plugin,&change.api,&outEvents);
    struct Reader{std::vector<uint8_t>*bytes;size_t offset=0;}reader{&bytes};clap_istream_t input{};input.ctx=&reader;input.read=[](const clap_istream_t*p,void*b,uint64_t size)->int64_t{auto&r=*static_cast<Reader*>(p->ctx);size=std::min<uint64_t>(size,r.bytes->size()-r.offset);std::memcpy(b,r.bytes->data()+r.offset,size);r.offset+=size;return size;};REQUIRE(state->load(plugin,&input));
    double restored=0.;REQUIRE(params->get_value(plugin,31,&restored)&&std::abs(restored-45./127.)<.00001);
    bytes.resize(64*sizeof(double));reader.offset=0;REQUIRE(state->load(plugin,&input));REQUIRE(params->get_value(plugin,64,&restored)&&restored==0.);REQUIRE(params->get_value(plugin,65,&restored)&&restored==0.);
    ec_daw_destroy(reference);plugin->destroy(plugin);std::printf("PASS CLAP %.0f Hz: transport, tempo, seek/loop, stop, variable blocks, latency, state; mode %d; peak %.6f\n",rate,mode,peak);
  }
  entry->deinit();
#ifdef _WIN32
  FreeLibrary(library);
#else
  dlclose(library);
#endif
}
int main(int argc,char**argv){
#ifdef _WIN32
  REQUIRE(argc==2);clap(argv[1]);
  // The Windows runner supplies an isolated APPDATA; test replacement of an
  // existing library as well as its first write (MoveFileEx semantics).
  REQUIRE(std::getenv("KEKLOCATOR_TEST_LIBRARY"));
  for(const char* saved:{"{\"patches\":[],\"favorites\":[]}","{\"patches\":[],\"favorites\":[\"factory-0\"]}"}){
    REQUIRE(ec_library_write(saved,std::strlen(saved))==0);
    std::array<char,512> restored{};
    REQUIRE(ec_library_read(restored.data(),restored.size())==std::strlen(saved));
    REQUIRE(std::strcmp(restored.data(),saved)==0);
  }
  std::puts("PASS Windows user library: first write, atomic replacement, read back");
#else
  REQUIRE(argc==3);au(argv[1]);clap(argv[2]);
#endif
  std::puts("PASS native format host tests");return 0;
}
