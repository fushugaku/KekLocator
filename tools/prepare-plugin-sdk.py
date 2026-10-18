#!/usr/bin/env python3
"""Fetch pinned public format SDKs into this project's private cache."""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCES = json.loads((ROOT / 'plugins/SOURCES.json').read_text(encoding='utf-8'))
VST_MARKERS = ('pluginterfaces/vst/ivstaudioprocessor.h',
               'base/source/fobject.cpp', 'cmake/modules/SMTG_Global.cmake',
               'public.sdk/source/main/dllmain.cpp')


def git(path, *args, **kwargs):
    return subprocess.run(['git', '-C', str(path), *args], check=True, **kwargs)


def checkout(name, path, markers):
    if all((path / marker).is_file() for marker in markers):
        return
    path.mkdir(parents=True, exist_ok=True)
    if not (path / '.git').exists():
        subprocess.run(['git', 'init', str(path)], check=True)
    source = SOURCES[name]
    remotes = subprocess.check_output(['git', '-C', str(path), 'remote'], text=True).splitlines()
    if 'origin' in remotes:
        git(path, 'remote', 'set-url', 'origin', source['url'])
    else:
        git(path, 'remote', 'add', 'origin', source['url'])
    git(path, 'fetch', '--depth', '1', 'origin', source['commit'])
    # iPlug2 tracks placeholder READMEs inside empty dependency directories.
    # Only project-private cache folders are replaced by these checkouts.
    git(path, 'checkout', '--force', '--detach', 'FETCH_HEAD')
    if 'submodules' in source:
        # Relative submodule URLs must resolve against the upstream origin.
        git(path, 'submodule', 'sync', '--recursive')
        git(path, 'submodule', 'update', '--init', '--depth', '1', *source['submodules'])


def main():
    if '--validator' in sys.argv[1:]:
        sdk = ROOT / '.tools/vst3-validation'
        checkout('VST3_SDK', sdk, VST_MARKERS)
        path = sdk / 'cmake/modules/SMTG_Global.cmake'
        text = path.read_text(encoding='utf-8')
        call = '\nsmtg_detect_xcode_version()\n'
        guard = '\nif(XCODE)\n    smtg_detect_xcode_version()\nendif()\n'
        if call in text:
            path.write_text(text.replace(call, guard, 1), encoding='utf-8')
        elif guard not in text:
            raise RuntimeError('Unexpected SDK Xcode detection; review the pinned source')
        return
    sdk = ROOT / '.tools/iplug2'
    checkout('iPlug2', sdk, ('iPlug2.cmake',))
    checkout('VST3_SDK', sdk / 'Dependencies/IPlug/VST3_SDK', VST_MARKERS)
    checkout('CLAP_SDK', sdk / 'Dependencies/IPlug/CLAP_SDK', ('include/clap/clap.h',))
    checkout('CLAP_HELPERS', sdk / 'Dependencies/IPlug/CLAP_HELPERS', ('include/clap/helpers/plugin.hh',))
    subprocess.run([sys.executable, str(ROOT / 'tools/patch-plugin-sdk.py')], check=True)
    print('Pinned iPlug2, VST3 and CLAP SDKs ready')


if __name__ == '__main__':
    main()
