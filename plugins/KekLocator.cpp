// Format, parameter and native WebView glue only. Audio/sequencer/SRC are Rust.
#include "KekLocator.h"
#include "IPlug_include_in_plug_src.h"
#include <algorithm>
#include <cmath>

using namespace iplug;
static constexpr int kParams=EC_PARAM_COUNT;
KekLocator::KekLocator(const InstanceInfo& info):iplug::Plugin(info,MakeConfig(kParams,1)) {
  float values[kParams];ec_daw_defaults(values);
  for(int i=0;i<kParams;i++){
    char name[64];ec_daw_name(i,name,sizeof(name));
    if(i==0||i==13||i==14||i==15||i==17||i==19||i==20||i==21||i==24||i==25||i==64||i==71)
      GetParam(i)->InitBool(name,values[i]>=.5);
    else GetParam(i)->InitDouble(name,values[i],0.,1.,.0000001);
    GetParam(i)->SetDisplayFunc([this,i](double value,WDL_String& str){
      float p[kParams];for(int j=0;j<kParams;j++)p[j]=GetParam(j)->GetNormalized();
      char display[128];ec_daw_display(i,value,p,mTempo.load(),display,sizeof(display));str.Set(display);
    });
    GetParam(i)->SetParseFunc([this,i](const char* text){float p[kParams];for(int j=0;j<kParams;j++)p[j]=GetParam(j)->GetNormalized();float value=p[i];ec_daw_parse(i,text,p,&value);return static_cast<double>(value);});
  }
#ifdef WEBVIEW_EDITOR_DELEGATE
  mEditorInitFunc=[this](){LoadIndexHtml(__FILE__,GetBundleID());EnableScroll(true);};
#endif
  MakeDefaultPreset("GREAT EYE",1);
  SetTailSize(48000*30);
}
KekLocator::~KekLocator(){ec_daw_destroy(mEngine);}
void KekLocator::OnActivate(bool active){if(active&&!mEngine)OnReset();}
void KekLocator::OnReset(){
  ec_daw_destroy(mEngine);mEngine=ec_daw_create(GetSampleRate());
  if(mEngine){
    TimedEvent stale;while(mPending.pop(stale)){}
    mFutureCount=0;mRampCount=0;for(auto& ramp:mRamps)ramp.active=false;
    for(int p=0;p<kParams;p++)ec_daw_set(mEngine,p,GetParam(p)->GetNormalized());
    SetLatency(ec_daw_latency(mEngine));SetTailSize(static_cast<int>(GetSampleRate()*30.));mError=0;
  }else mError=1;
}
void KekLocator::ProcessBlock(sample** in,sample** out,int nFrames){
  if(!mEngine){for(int c=0;c<NOutChansConnected();c++)std::fill(out[c],out[c]+nFrames,0.);return;}
  size_t eventCount=mFutureCount;std::copy_n(mFuture.begin(),mFutureCount,mEvents.begin());mFutureCount=0;while(eventCount<mEvents.size()&&mPending.pop(mEvents[eventCount]))++eventCount;
  std::sort(mEvents.begin(),mEvents.begin()+eventCount,[](const TimedEvent&a,const TimedEvent&b){return a.offset!=b.offset?a.offset<b.offset:a.order<b.order;});
  size_t nextEvent=0;
  const auto tempo=GetTempo()>0.?GetTempo():120.;
  const auto beat=GetPPQPos();const bool playing=GetTransportIsRunning();
  mPlaying=playing;float peakL=0.f,peakR=0.f;
  for(int offset=0;offset<nFrames;){
    while(nextEvent<eventCount&&mEvents[nextEvent].offset<=offset)Apply(mEvents[nextEvent++]);
    const int boundary=nextEvent<eventCount?std::clamp(mEvents[nextEvent].offset,offset,nFrames):nFrames;
    if(mRampCount){for(int p=0;p<kParams;p++){auto& ramp=mRamps[p];if(ramp.active){
      const float fraction=std::min(1.f,static_cast<float>(ramp.elapsed)/std::max(1,ramp.duration));
      const float value=ramp.begin+(ramp.end-ramp.begin)*fraction;ec_daw_set(mEngine,p,value);
      GetParam(p)->SetNormalized(value);
      if(ramp.elapsed>=ramp.duration){ramp.active=false;--mRampCount;}else ++ramp.elapsed;
    }}}
    const int n=std::min(mRampCount?1:512,boundary-offset);
    if(n==0)break;
    for(int i=0;i<n;i++){
      mAudio[i*2]=NInChansConnected()>0?in[0][offset+i]:0.;
      mAudio[i*2+1]=NInChansConnected()>1?in[1][offset+i]:mAudio[i*2];
    }
    int err=ec_daw_process(mEngine,mAudio.data(),n,tempo,beat+offset*tempo/(60.*GetSampleRate()),playing);
    if(err){mAudio.fill(0.f);mError=err;}
    for(int i=0;i<n;i++){
      if(NOutChansConnected()==1)out[0][offset+i]=(mAudio[i*2]+mAudio[i*2+1])*.5;
      else {out[0][offset+i]=mAudio[i*2];out[1][offset+i]=mAudio[i*2+1];}
      peakL=std::max(peakL,std::abs(mAudio[i*2]));peakR=std::max(peakR,std::abs(mAudio[i*2+1]));
    }
    offset+=n;
  }
  while(nextEvent<eventCount){auto event=mEvents[nextEvent++];event.offset-=nFrames;
    if(mFutureCount<mFuture.size())mFuture[mFutureCount++]=event;else mError=2;
  }
  mStep=ec_daw_step(mEngine);mTempo=ec_daw_tempo(mEngine);mPeakL=peakL;mPeakR=peakR;mDuck=ec_daw_duck(mEngine);
}
void KekLocator::OnParamChange(int id,EParamSource source,int sampleOffset){
  if(mPresetGesture.load()&&source==kUI)return;
  TimedEvent e;e.id=id;e.value=GetParam(id)->GetNormalized();e.offset=std::max(0,sampleOffset);Queue(e);
}
void KekLocator::OnParamRamp(int id,int offset,int duration,double begin,double end){
  if(id<0||id>=kParams)return;
  TimedEvent e;e.kind=TimedEvent::Ramp;e.id=id;e.offset=std::max(0,offset);e.duration=std::max(0,duration+std::min(0,offset));
  const double elapsed=duration>0?std::clamp(-static_cast<double>(offset)/duration,0.,1.):1.;
  e.value=begin+(end-begin)*elapsed;e.end=end;Queue(e);
}
void KekLocator::ProcessMidiMsg(const IMidiMsg& m){
  TimedEvent e;e.kind=TimedEvent::Midi;e.offset=std::max(0,m.mOffset);e.status=m.mStatus;e.data1=m.mData1;e.data2=m.mData2;Queue(e);
}
void KekLocator::Apply(const TimedEvent& e){
  if(e.kind==TimedEvent::Ramp){auto& r=mRamps[e.id];if(!r.active)++mRampCount;r={true,e.duration,0,e.value,e.end};return;}
  if(e.kind==TimedEvent::Parameter){if(mRamps[e.id].active){mRamps[e.id].active=false;--mRampCount;}if(ec_daw_set(mEngine,e.id,e.value))mError=3;return;}
  if(e.kind==TimedEvent::Preset){if(ec_daw_preset(mEngine,e.preset.data(),e.value))mError=3;return;}
  static constexpr int map[13]={3,10,4,5,6,2,1,18,9,7,8,11,19};
  int id=-1;float value=0.f;const auto status=e.status&0xf0;
  if(status==0xb0&&e.data1>=70&&e.data1<=82){id=map[e.data1-70];value=id==19?(e.data2>=64?1.f:0.f):ec_daw_cc_value(e.data2);}
  else if(status==0x90&&e.data2){id=3;value=ec_daw_note_time(e.data1);}
  else if(status==0xc0){float v[20];if(ec_daw_program(mEngine,e.data1,v)==0){for(int p=0;p<20;p++)GetParam(p)->SetNormalized(v[p]);mMidiDirty.fetch_or((uint64_t(1)<<20)-1);}}
  if(id>=0){GetParam(id)->SetNormalized(value);ec_daw_set(mEngine,id,value);mMidiDirty.fetch_or(uint64_t(1)<<id);}
}
// 1.0 stored exactly 64 doubles; append-only parameters preserve every old ID.
int KekLocator::UnserializeState(const IByteChunk& chunk,int startPos){
  const int bytes=chunk.Size()-startPos;
  const int count=(bytes==64*static_cast<int>(sizeof(double))||bytes==64*static_cast<int>(sizeof(double))+4)?64:kParams;
  if(bytes<count*static_cast<int>(sizeof(double)))return -1;
  std::array<double,kParams> values{};float defaults[kParams];ec_daw_defaults(defaults);
  for(int p=0;p<kParams;p++)values[p]=defaults[p];
  int pos=startPos;
  for(int p=0;p<count;p++){pos=chunk.Get(&values[p],pos);if(pos<0||!std::isfinite(values[p])||values[p]<0.||values[p]>1.)return -1;}
  for(int p=0;p<kParams;p++)GetParam(p)->SetNormalized(values[p]);
  OnParamReset(kPresetRecall);return pos;
}
void KekLocator::SendSnapshot(){
#ifdef WEBVIEW_EDITOR_DELEGATE
  nlohmann::json values=nlohmann::json::array();for(int p=0;p<kParams;p++)values.push_back(GetParam(p)->GetNormalized());
  SendJSONFromDelegate({{"type","parameters"},{"values",values}});
#endif
}
void KekLocator::OnUIOpen(){SendSnapshot();}
void KekLocator::OnIdle(){
#ifdef WEBVIEW_EDITOR_DELEGATE
  if(!mEditorReady)return;
  auto dirty=mMidiDirty.exchange(0);
  if(dirty)SendSnapshot();
  SendJSONFromDelegate({{"type","transport"},{"tempo",mTempo.load()},{"playing",mPlaying.load()},
    {"step",mStep.load()},{"left",mPeakL.load()},{"right",mPeakR.load()},{"error",mError.load()},{"duck",mDuck.load()}});
#endif
}
bool KekLocator::OnMessage(int tag,int,int size,const void* data){
  if(tag==101){mEditorReady=true;SendSnapshot();
#ifdef WEBVIEW_EDITOR_DELEGATE
    int length=ec_library_read(nullptr,0);if(length>0&&length<=1048577){std::vector<char> bytes(length);if(ec_library_read(bytes.data(),length)>=0){auto library=nlohmann::json::parse(bytes.data(),nullptr,false);if(!library.is_discarded())SendJSONFromDelegate({{"type","library"},{"library",library}});}}
#endif
    return true;}
  if(tag==102&&size>0&&size<=1048576){int result=ec_library_write(static_cast<const char*>(data),size);
#ifdef WEBVIEW_EDITOR_DELEGATE
    SendJSONFromDelegate({{"type","status"},{"message",result==0?"Library saved":"Could not save library"}});
#endif
    return result==0;}
  if(tag==100&&size>0&&size<16384){
    auto j=nlohmann::json::parse(static_cast<const char*>(data),static_cast<const char*>(data)+size,nullptr,false);
    if(j.is_discarded())return false;
    if(j.value("type","")=="preset"&&j.contains("values")&&j["values"].size()==20){
      for(int p=0;p<20;p++)if(!j["values"][p].is_number())return false;
      TimedEvent event;event.kind=TimedEvent::Preset;event.value=std::clamp(j.value("seconds",0.f),0.f,10.f);
      mPresetGesture=true;
      for(int p=0;p<20;p++){
        double v=std::clamp(j["values"][p].get<double>(),0.,1.);
        event.preset[p]=v;
        BeginInformHostOfParamChangeFromUI(p);SendParameterValueFromUI(p,v);EndInformHostOfParamChangeFromUI(p);
      }
      mPresetGesture=false;Queue(event);return true;
    }
  }
  return false;
}
