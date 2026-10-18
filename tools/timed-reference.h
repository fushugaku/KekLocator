#pragma once
#include <vector>
#include <algorithm>
#include "../plugins/engine.h"
struct EcPoint {int offset,id;float value;};
inline void timed_reference(void* reference,std::vector<float>& audio,int n,double rate,double bpm,double beat,const std::vector<EcPoint>& points){
  std::fill(audio.begin(),audio.end(),0.f);int offset=0;
  for(const auto& point:points){int count=point.offset-offset;if(count>0)REQUIRE(ec_daw_process(reference,audio.data()+offset*2,count,bpm,beat+offset/rate*bpm/60.,1)==0);
    REQUIRE(ec_daw_set(reference,point.id,point.value)==0);offset=point.offset;}
  if(offset<n)REQUIRE(ec_daw_process(reference,audio.data()+offset*2,n-offset,bpm,beat+offset/rate*bpm/60.,1)==0);
}
