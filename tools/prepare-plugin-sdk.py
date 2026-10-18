#!/usr/bin/env python3
"""Fetch pinned public format SDKs inside this project; never touches other ports."""
import json,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
sources=json.loads((ROOT/'plugins/SOURCES.json').read_text())
def checkout(name,path,marker):
    if (path/marker).is_file():return
    path.mkdir(parents=True,exist_ok=True)
    if not(path/'.git').exists():subprocess.run(['git','init',str(path)],check=True)
    source=sources[name]
    subprocess.run(['git','-C',str(path),'fetch','--depth','1',source['url'],source['commit']],check=True)
    # iPlug2 tracks placeholder READMEs inside empty dependency folders.
    # These private cache folders are replaced by the pinned dependency checkout.
    subprocess.run(['git','-C',str(path),'checkout','--force','--detach','FETCH_HEAD'],check=True)
    if 'submodules'in source:subprocess.run(['git','-C',str(path),'submodule','update','--init','--depth','1',*source['submodules']],check=True)
sdk=ROOT/'.tools/iplug2'
checkout('iPlug2',sdk,'iPlug2.cmake')
checkout('VST3_SDK',sdk/'Dependencies/IPlug/VST3_SDK','pluginterfaces/vst/ivstaudioprocessor.h')
checkout('CLAP_SDK',sdk/'Dependencies/IPlug/CLAP_SDK','include/clap/clap.h')
checkout('CLAP_HELPERS',sdk/'Dependencies/IPlug/CLAP_HELPERS','include/clap/helpers/plugin.hh')
subprocess.run([sys.executable,str(ROOT/'tools/patch-plugin-sdk.py')],check=True)
print('Pinned iPlug2, VST3 and CLAP SDKs ready')
