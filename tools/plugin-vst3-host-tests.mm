// Exercise the shipped VST3 binary with genuine SDK transport/parameter APIs.
#ifdef _WIN32
#define NOMINMAX
#include <windows.h>
#else
#include <CoreFoundation/CoreFoundation.h>
#endif
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <vector>
#include "pluginterfaces/base/ipluginbase.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "pluginterfaces/vst/ivstcomponent.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include "pluginterfaces/vst/ivstprocesscontext.h"
#include "public.sdk/source/common/memorystream.h"
#include "public.sdk/source/vst/hosting/hostclasses.h"
#include "public.sdk/source/vst/hosting/parameterchanges.h"
#include "../plugins/engine.h"

using namespace Steinberg;
using namespace Steinberg::Vst;
#define REQUIRE(x) do { if (!(x)) { std::fprintf(stderr,"FAIL line %d: %s\n",__LINE__,#x); std::exit(1); } } while (0)
#include "timed-reference.h"
static void change(ParameterChanges& changes, ParamID id, double value,int offset=0) {
  int32 index=0; auto queue=changes.addParameterData(id,index);
  REQUIRE(queue&&queue->addPoint(offset,value,index)==kResultOk);
}
int main(int argc,char** argv) {
  REQUIRE(argc==2);
#ifdef _WIN32
  auto bundle=LoadLibraryA(argv[1]); REQUIRE(bundle);
  auto enter=reinterpret_cast<bool(*)()>(GetProcAddress(bundle,"InitDll"));
  auto leave=reinterpret_cast<bool(*)()>(GetProcAddress(bundle,"ExitDll"));
  auto getFactory=reinterpret_cast<IPluginFactory*(*)()>(GetProcAddress(bundle,"GetPluginFactory"));
  REQUIRE(enter&&leave&&getFactory&&enter());
#else
  auto url=CFURLCreateFromFileSystemRepresentation(nullptr,reinterpret_cast<const UInt8*>(argv[1]),std::strlen(argv[1]),true);
  auto bundle=CFBundleCreate(nullptr,url); REQUIRE(bundle&&CFBundleLoadExecutable(bundle));
  auto enter=reinterpret_cast<bool(*)(CFBundleRef)>(CFBundleGetFunctionPointerForName(bundle,CFSTR("bundleEntry")));
  auto leave=reinterpret_cast<bool(*)()>(CFBundleGetFunctionPointerForName(bundle,CFSTR("bundleExit")));
  auto getFactory=reinterpret_cast<IPluginFactory*(*)()>(CFBundleGetFunctionPointerForName(bundle,CFSTR("GetPluginFactory")));
  REQUIRE(enter&&leave&&getFactory&&enter(bundle));
#endif
  auto factory=owned(getFactory()); REQUIRE(factory);
  PClassInfo info{}; REQUIRE(factory->getClassInfo(0,&info)==kResultOk);
  REQUIRE(std::strcmp(info.name,"KekLocator")==0);
  HostApplication host;
  for(int mode:{0,1})for(double rate:{44100.,48000.,96000.,192000.}) {
    IComponent* raw=nullptr;
    REQUIRE(factory->createInstance(info.cid,IComponent::iid,reinterpret_cast<void**>(&raw))==kResultOk);
    auto component=owned(raw); REQUIRE(component->initialize(&host)==kResultOk);
    FUnknownPtr<IAudioProcessor> processor(component); FUnknownPtr<IEditController> controller(component);
    REQUIRE(processor&&controller);
    SpeakerArrangement stereo=SpeakerArr::kStereo;
    REQUIRE(processor->setBusArrangements(&stereo,1,&stereo,1)==kResultOk);
    REQUIRE(component->activateBus(kAudio,kInput,0,true)==kResultOk);
    REQUIRE(component->activateBus(kAudio,kOutput,0,true)==kResultOk);
    ProcessSetup setup{kRealtime,kSample32,1024,rate};
    REQUIRE(processor->setupProcessing(setup)==kResultOk);
    REQUIRE(component->setActive(true)==kResultOk);
    REQUIRE(processor->setProcessing(true)==kResultOk);
    void* reference=ec_daw_create(rate); REQUIRE(reference);
    std::array<float,EC_PARAM_COUNT> values{};ec_daw_defaults(values.data());
    values[24]=1.; values[18]=0.; values[21]=0.;values[64]=mode;
    if(mode){values[65]=0.8f;values[69]=0.7f;values[71]=1.f;for(int s=0;s<16;s++){values[72+s]=0.65f;values[88+s]=0.6f;values[104+s]=3.f/7.f;}values[120]=4.f/31.f;values[138]=0.3f;}
    ParameterChanges changes;
    for(int p=0;p<EC_PARAM_COUNT;p++) { change(changes,p,values[p]); REQUIRE(ec_daw_set(reference,p,values[p])==0); }
    REQUIRE(processor->getLatencySamples()==ec_daw_latency(reference));
    std::array<float,1024> left{},right{},zero{};
    float* ins[2]={zero.data(),zero.data()};float* outs[2]={left.data(),right.data()};
    AudioBusBuffers input{},output{};input.numChannels=output.numChannels=2;input.channelBuffers32=ins;output.channelBuffers32=outs;
    std::vector<float> expected(2048);double peak=0.;int64 sample=0;
    for(int phase=0;phase<6;phase++) {
      double bpm=phase==2?160.:phase==3?90.:120.,beat=phase==3?.125:phase*.75;
      bool playing=phase!=4;
      for(int n:{1,17,128,511,1024}) {
        ProcessContext context{};context.state=ProcessContext::kTempoValid|ProcessContext::kProjectTimeMusicValid|(playing?ProcessContext::kPlaying:0);
        context.sampleRate=rate;context.projectTimeSamples=sample;context.projectTimeMusic=beat;context.tempo=bpm;
        ProcessData data{};data.processMode=kRealtime;data.symbolicSampleSize=kSample32;data.numSamples=n;data.numInputs=data.numOutputs=1;
        data.inputs=&input;data.outputs=&output;data.inputParameterChanges=&changes;data.processContext=&context;
        std::fill(expected.begin(),expected.end(),0.f);
        REQUIRE(ec_daw_process(reference,expected.data(),n,bpm,beat,playing)==0);
        REQUIRE(processor->process(data)==kResultOk);
        for(int i=0;i<n;i++)for(int c=0;c<2;c++) {
          float actual=outs[c][i],want=expected[i*2+c];
          if(!std::isfinite(actual)||std::abs(actual-want)>0.000002f) {
            std::fprintf(stderr,"%.0f Hz sample %d/%d actual %.9g reference %.9g\n",rate,i,c,actual,want);REQUIRE(false);
          }
          peak=std::max(peak,std::abs(double(actual)));
        }
        changes.clearQueue(); sample+=n;beat+=n/rate*bpm/60.;
      }
    }
    // Three points in one queue must all reach DSP at their own sample offsets.
    {const int n=511;const double beat=4.;
      std::vector<EcPoint> points{{5,23,.2f},{127,23,.75f},{300,23,.4f}};
      for(const auto& p:points)change(changes,p.id,p.value,p.offset);
      timed_reference(reference,expected,n,rate,120.,beat,points);
      ProcessContext context{};context.state=ProcessContext::kTempoValid|ProcessContext::kProjectTimeMusicValid|ProcessContext::kPlaying;context.sampleRate=rate;context.projectTimeMusic=beat;context.tempo=120.;
      ProcessData data{};data.processMode=kRealtime;data.symbolicSampleSize=kSample32;data.numSamples=n;data.numInputs=data.numOutputs=1;data.inputs=&input;data.outputs=&output;data.inputParameterChanges=&changes;data.processContext=&context;
      REQUIRE(processor->process(data)==kResultOk);
      for(int i=0;i<n;i++)for(int c=0;c<2;c++)REQUIRE(std::abs(outs[c][i]-expected[i*2+c])<0.000002f);
      changes.clearQueue();
      String128 display{};REQUIRE(controller->getParamStringByValue(22,.6,display)==kResultOk);REQUIRE(display[0]=='-'&&display[1]=='6');
      String128 typed{};const char* physical="-6 dB";for(int i=0;physical[i];i++)typed[i]=physical[i];ParamValue parsed=0.;REQUIRE(controller->getParamValueByString(22,typed,parsed)==kResultOk);REQUIRE(std::abs(parsed-.6)<.00001);
    }
    REQUIRE(peak>0.001);
    REQUIRE(processor->setProcessing(false)==kResultOk); REQUIRE(component->setActive(false)==kResultOk);
    MemoryStream saved;REQUIRE(component->getState(&saved)==kResultOk);
    REQUIRE(controller->setParamNormalized(31,.9)==kResultOk);
    REQUIRE(saved.seek(0,IBStream::kIBSeekSet,nullptr)==kResultOk);
    REQUIRE(component->setState(&saved)==kResultOk);
    REQUIRE(std::abs(controller->getParamNormalized(31)-45./127.)<0.00001);
    // A 1.0 VST3 state is 64 doubles plus SDK bypass; extensions reset to defaults.
    MemoryStream legacy;float defaults[EC_PARAM_COUNT];ec_daw_defaults(defaults);
    for(int p=0;p<64;p++){double v=defaults[p];REQUIRE(legacy.write(&v,sizeof(v),nullptr)==kResultOk);}int32 bypass=0;REQUIRE(legacy.write(&bypass,sizeof(bypass),nullptr)==kResultOk);
    REQUIRE(legacy.seek(0,IBStream::kIBSeekSet,nullptr)==kResultOk);REQUIRE(component->setState(&legacy)==kResultOk);REQUIRE(controller->getParamNormalized(64)==0.);REQUIRE(controller->getParamNormalized(65)==0.);
    ec_daw_destroy(reference);REQUIRE(component->terminate()==kResultOk);
    std::printf("PASS VST3 %.0f Hz: transport, tempo, seek/loop, stop, variable blocks, latency, state; mode %d; peak %.6f\n",rate,mode,peak);
  }
  factory=nullptr;REQUIRE(leave());
#ifdef _WIN32
  FreeLibrary(bundle);
#else
  CFRelease(bundle);CFRelease(url);
#endif
}
