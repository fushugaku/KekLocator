#!/usr/bin/env python3
"""Small, idempotent patches against the pinned, project-private iPlug2 checkout."""
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
SDK=ROOT/'.tools/iplug2'
def patch(name, old, new):
    p=SDK/name;s=p.read_text()
    if new in s:return
    if s.count(old)!=1:raise RuntimeError(f'Pinned SDK changed: {name}')
    p.write_text(s.replace(old,new))
patch('IPlug/VST3/IPlugVST3_ProcessorBase.cpp',
'''        if (paramQueue->getPoint(numPoints - 1,  offsetSamples, value) == kResultTrue)
        {''',
'''        // Echolocator: deliver every timestamped point, not only the final value.
        for (int32 point = 0; point < numPoints; ++point)
        if (paramQueue->getPoint(point, offsetSamples, value) == kResultTrue)
        {''')
patch('IPlug/IPlugParameter.h',
'  DisplayFunc mDisplayFunction = nullptr;',
'  DisplayFunc mDisplayFunction = nullptr;\n  std::function<double(const char*)> mParseFunction = nullptr;')
patch('IPlug/IPlugParameter.h',
'  void SetDisplayFunc(DisplayFunc func) { mDisplayFunction = func; }',
'  void SetDisplayFunc(DisplayFunc func) { mDisplayFunction = func; }\n  void SetParseFunc(std::function<double(const char*)> func) { mParseFunction = func; }')
patch('IPlug/IPlugParameter.cpp',
'''double IParam::StringToValue(const char* str) const
{''',
'''double IParam::StringToValue(const char* str) const
{
  // Echolocator: typed values use the same physical units as host displays.
  if (mParseFunction) return Constrain(mParseFunction(str));''')
print('Applied timestamp and physical-value parser patches to the private SDK')
patch('IPlug/IPlugEditorDelegate.h',
'  virtual void OnParamChange(int paramIdx) {}',
'''  virtual void OnParamChange(int paramIdx) {}
  // Echolocator: preserve AU scheduled ramps as ramps rather than discarding them.
  virtual void OnParamRamp(int paramIdx, int offset, int duration, double begin, double end) {}''')
patch('IPlug/AUv2/IPlugAU.cpp',
'''      if (r != noErr)
      {
        return r;
      }
    }
  }
  return noErr;
}

//static
OSStatus IPlugAU::DoRender''',
'''      if (r != noErr)
      {
        return r;
      }
    }
    else if (pEvent->eventType == kParameterEvent_Ramped)
    {
      if (pEvent->parameter >= _this->NParams()) return kAudioUnitErr_InvalidParameter;
      const auto& ramp = pEvent->eventValues.ramp;
      _this->OnParamRamp(pEvent->parameter, ramp.startBufferOffset,
                        ramp.durationInFrames, ramp.startValue, ramp.endValue);
    }
  }
  return noErr;
}

//static
OSStatus IPlugAU::DoRender''')

patch('IPlug/IPlug_include_in_plug_src.h',
'  gPluginDesc->id = BUNDLE_DOMAIN "." BUNDLE_MFR "." BUNDLE_NAME;',
'  #ifdef PLUG_CLAP_ID\n  gPluginDesc->id = PLUG_CLAP_ID;\n  #else\n  gPluginDesc->id = BUNDLE_DOMAIN "." BUNDLE_MFR "." BUNDLE_NAME;\n  #endif')
