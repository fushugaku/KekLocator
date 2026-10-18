#pragma once
#include "IPlug_include_in_plug_hdr.h"
#include "engine.h"
#include <array>
#include <atomic>
#include "TimedEvents.h"

class KekLocator final:public iplug::Plugin {
public:
  explicit KekLocator(const iplug::InstanceInfo&);
  ~KekLocator() override;
  void ProcessBlock(iplug::sample**,iplug::sample**,int) override;
  void ProcessMidiMsg(const iplug::IMidiMsg&) override;
  void OnReset() override;
  void OnParamChange(int,iplug::EParamSource,int=-1) override;
  void OnParamRamp(int,int,int,double,double) override;
  int UnserializeState(const iplug::IByteChunk&,int) override;
  void OnActivate(bool) override;
  void OnIdle() override;
  void OnUIOpen() override;
  void CloseWindow() override {mEditorReady=false;iplug::Plugin::CloseWindow();}
  bool OnMessage(int,int,int,const void*) override;
private:
  void* mEngine=nullptr;
  std::array<float,1024> mAudio{};
  TimedEvents mPending;
  std::array<TimedEvent,4096> mEvents;
  std::array<TimedEvent,4096> mFuture;size_t mFutureCount=0;
  struct Ramp {bool active=false;int duration=0,elapsed=0;float begin=0.f,end=0.f;};
  std::array<Ramp,EC_PARAM_COUNT> mRamps;int mRampCount=0;
  std::atomic<bool> mPresetGesture{false};
  std::atomic<float> mDuck{0.f};
  void Queue(TimedEvent event){if(!mPending.push(event))mError=2;}
  void Apply(const TimedEvent&);
  void SendSnapshot();
  std::atomic<int> mStep{-1};
  std::atomic<double> mTempo{120.};
  std::atomic<bool> mPlaying{false};
  std::atomic<float> mPeakL{0.f},mPeakR{0.f};
  std::atomic<int> mError{0};
  std::atomic<bool> mEditorReady{false};
  std::atomic<uint64_t> mMidiDirty{0};
};
