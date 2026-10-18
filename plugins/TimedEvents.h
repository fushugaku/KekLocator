#pragma once
#include <array>
#include <atomic>
#include <cstdint>
#include <cstddef>
// Bounded multi-producer queue: UI/host callbacks never wait on the audio thread.
struct TimedEvent {
  enum Kind {Parameter, Midi, Preset, Ramp};
  Kind kind=Parameter; int offset=0, id=0; float value=0.f;
  std::array<float,20> preset{}; unsigned char status=0,data1=0,data2=0;
  uint64_t order=0; int duration=0;float end=0.f;
};
class TimedEvents {
  static constexpr size_t Capacity=4096;
  struct Slot {std::atomic<size_t> sequence{0};TimedEvent event;};
  std::array<Slot,Capacity> mSlots;
  std::atomic<size_t> mWrite{0};size_t mRead=0;
public:
  TimedEvents(){for(size_t i=0;i<Capacity;i++)mSlots[i].sequence.store(i);}
  bool push(TimedEvent event){
    size_t pos=mWrite.load(std::memory_order_relaxed);
    for(;;){auto& slot=mSlots[pos%Capacity];size_t seq=slot.sequence.load(std::memory_order_acquire);
      const intptr_t difference=static_cast<intptr_t>(seq)-static_cast<intptr_t>(pos);
      if(difference==0){if(mWrite.compare_exchange_weak(pos,pos+1,std::memory_order_relaxed)){
        event.order=pos;slot.event=event;slot.sequence.store(pos+1,std::memory_order_release);return true;}}
      else if(difference<0)return false;else pos=mWrite.load(std::memory_order_relaxed);
    }
  }
  bool pop(TimedEvent& event){auto& slot=mSlots[mRead%Capacity];if(slot.sequence.load(std::memory_order_acquire)!=mRead+1)return false;
    event=slot.event;slot.sequence.store(mRead+Capacity,std::memory_order_release);++mRead;return true;}
};
