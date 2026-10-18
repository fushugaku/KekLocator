#!/usr/bin/env python3
"""Inspect installer choices and compare every extracted plug-in file to its source."""
import hashlib
import json
import plistlib
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FORMATS = [('VST3', 'vst3', 'vst3', 'VST3'),
           ('AU', 'component', 'au', 'Components'),
           ('CLAP', 'clap', 'clap', 'CLAP')]


def output(*args):
    return subprocess.check_output([str(arg) for arg in args])


def digests(bundle):
    return {str(path.relative_to(bundle)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in bundle.rglob('*') if path.is_file()}


def main():
    package = Path(sys.argv[1]).resolve()
    domains = output('installer', '-dominfo', '-pkg', package).decode()
    assert 'CurrentUserHomeDirectory' in domains and 'LocalSystem' not in domains, domains
    choices_bytes = output('installer', '-showChoicesXML', '-pkg', package,
                           '-target', 'CurrentUserHomeDirectory')
    def leaves(nodes):
        for node in nodes:
            children = node.get('childItems', [])
            if children:
                yield from leaves(children)
            else:
                yield node
    choices = list(leaves(plistlib.loads(choices_bytes)))
    assert {choice['choiceIdentifier'] for choice in choices} == {'vst3', 'au', 'clap'}
    assert all(choice['choiceIsEnabled'] and choice['choiceIsSelected'] for choice in choices)
    verified = []
    with tempfile.TemporaryDirectory(prefix='echolocator-pkg-check-', dir=ROOT / 'target') as temp:
        expanded = Path(temp) / 'expanded'
        subprocess.run(['pkgutil', '--expand-full', str(package), str(expanded)], check=True)
        distribution = ET.parse(expanded / 'Distribution').getroot()
        assert distribution.find('volume-check/allowed-os-versions/os-version').get('min') == '11.0'
        assert set(distribution.find('options').get('hostArchitectures').split(',')) == {'arm64', 'x86_64'}
        for label, suffix, identifier, folder in FORMATS:
            component = expanded / f'KekLocator-{label}.pkg'
            info = ET.parse(component / 'PackageInfo').getroot()
            assert info.get('identifier') == f'local.ErikaRust.pkg.KekLocator.{identifier}'
            assert info.get('install-location') == f'/Library/Audio/Plug-Ins/{folder}'
            assert info.find('scripts') is None and not (component / 'Scripts').exists()
            bundle_name = f'KekLocator.{suffix}'
            bundle = component / 'Payload' / bundle_name
            source = ROOT / 'build/plugins/macos-universal' / bundle_name
            source_files = digests(source)
            assert source_files == digests(bundle), f'Changed payload in {bundle_name}'
            subprocess.run(['codesign', '--verify', '--strict', str(bundle)], check=True)
            architectures = output('lipo', '-archs', bundle / 'Contents/MacOS/KekLocator').decode().split()
            assert set(architectures) == {'arm64', 'x86_64'}
            assert info.get('relocatable') == 'false'
            assert info.find('relocate') is not None and len(info.find('relocate')) == 0
            verified.append({'format': label, 'version': info.get('version'),
                             'path': f'~/Library/Audio/Plug-Ins/{folder}/{bundle_name}',
                             'files_compared': len(source_files), 'architectures': architectures,
                             'payload_matches_source': True, 'code_signature': 'PASS'})
    report = {'status': 'PASS', 'package': str(package), 'bytes': package.stat().st_size,
              'sha256': hashlib.sha256(package.read_bytes()).hexdigest(),
              'installation_domain': 'CurrentUserHomeDirectory', 'minimum_macos': '11.0',
              'install_scripts': False, 'components': verified,
              'verification': 'Installer metadata and format choices evaluated; all extracted bundle files match verified build; installation was not executed.'}
    (ROOT / 'research').mkdir(exist_ok=True)
    (ROOT / 'research/pkg-verification.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(f'PASS: installer choices, user-home paths, {sum(c["files_compared"] for c in verified)} matching files, universal code signatures')
    print(f'SHA256: {report["sha256"]}')


if __name__ == '__main__':
    main()
