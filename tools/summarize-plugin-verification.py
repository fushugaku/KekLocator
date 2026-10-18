#!/usr/bin/env python3
"""Summarize native macOS evidence from the current build; no installation claim."""
import hashlib
import json
import plistlib
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
research = ROOT / 'research'
def read(name):
    return (research / name).read_text()
native = read('plugin-host-tests.log') + read('plugin-vst3-host-tests.log')
cases = re.findall(r'^PASS (AU|CLAP|VST3) (\d+) Hz:.*mode (\d)', native, re.M)
assert len(cases) == 24 and len(set(cases)) == 24
validator = re.findall(r'Result: (\d+) tests passed, (\d+) tests failed', read('plugin-vst3-validation.log'))
assert validator and validator[-1][1] == '0'
rust = sum(int(n) for n in re.findall(r'test result: ok\. (\d+) passed', read('plugin-final-tests.log')))
assert rust >= 25
bundles = []
for suffix in ('vst3', 'component', 'clap'):
    bundle = ROOT / 'build/plugins/macos-universal' / f'KekLocator.{suffix}'
    info = plistlib.loads((bundle / 'Contents/Info.plist').read_bytes())
    assert info['CFBundleShortVersionString'] == '1.2.0'
    binary = bundle / 'Contents/MacOS/KekLocator'
    bundles.append({'format': suffix, 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest()})
report = {'product': 'KekLocator', 'version': '1.2.0', 'status': 'PASS',
          'platform': 'macOS universal; runtime tested on arm64', 'rust_tests': rust,
          'native_render_cases': len(cases), 'absolute_audio_tolerance': 0.000002,
          'vst3_validator': {'passed': int(validator[-1][0]), 'failed': 0}, 'bundles': bundles,
          'limits': ['No exhaustive DAW product/bounce claim.', 'Analog circuitry is not modeled.']}
(research / 'plugin-verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
