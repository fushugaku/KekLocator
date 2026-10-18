#!/usr/bin/env python3
"""Package the verified universal binaries, without modifying their signatures."""
import argparse
import plistlib
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FORMATS = [('VST3', 'vst3', 'vst3', 'VST3'),
           ('AU', 'component', 'au', 'Components'),
           ('CLAP', 'clap', 'clap', 'CLAP')]


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sign', help='Optional Developer ID Installer identity')
    args = parser.parse_args()
    source = ROOT / 'build/plugins/macos-universal'
    versions = set()
    for _, suffix, _, _ in FORMATS:
        bundle = source / f'KekLocator.{suffix}'
        with (bundle / 'Contents/Info.plist').open('rb') as f:
            versions.add(plistlib.load(f)['CFBundleShortVersionString'])
        run('codesign', '--verify', '--strict', bundle)
        architectures = subprocess.check_output(
            ['lipo', '-archs', str(bundle / 'Contents/MacOS/KekLocator')], text=True).split()
        if set(architectures) != {'arm64', 'x86_64'}:
            raise SystemExit(f'Expected universal bundle: {bundle}')
    if len(versions) != 1:
        raise SystemExit('The three plug-in versions do not match')
    version = versions.pop()
    output = ROOT / 'build/plugins' / f'KekLocator-{version}-macOS-universal.pkg'
    (ROOT / 'target').mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='echolocator-pkg-', dir=ROOT / 'target') as temp:
        stage = Path(temp)
        packages = stage / 'packages'
        packages.mkdir()
        resources = stage / 'resources'
        resources.mkdir()
        for label, suffix, identifier, folder in FORMATS:
            payload = stage / identifier
            payload.mkdir()
            bundle = f'KekLocator.{suffix}'
            run('ditto', '--norsrc', '--noextattr', source / bundle, payload / bundle)
            run('codesign', '--verify', '--strict', payload / bundle)
            components = stage / f'{identifier}.plist'
            components.write_bytes(plistlib.dumps([{
                'RootRelativeBundlePath': bundle,
                'BundleIsRelocatable': False,
                'BundleIsVersionChecked': True,
                'BundleHasStrictIdentifier': True,
                'BundleOverwriteAction': 'upgrade',
            }]))
            run('pkgbuild', '--root', payload, '--component-plist', components,
                '--identifier', f'local.ErikaRust.pkg.KekLocator.{identifier}',
                '--version', version, '--install-location', f'/Library/Audio/Plug-Ins/{folder}',
                '--ownership', 'recommended', '--compression', 'legacy',
                packages / f'KekLocator-{label}.pkg')
        templates = ROOT / 'plugins/installer'
        distribution = stage / 'Distribution.xml'
        distribution.write_text((templates / 'Distribution.xml').read_text().replace('@VERSION@', version))
        for name in ['welcome.html', 'conclusion.html']:
            (resources / name).write_text((templates / name).read_text().replace('@VERSION@', version))
        command = ['productbuild', '--distribution', distribution, '--resources', resources,
                   '--package-path', packages, '--identifier', 'local.ErikaRust.pkg.KekLocator',
                   '--version', version]
        if args.sign:
            command += ['--sign', args.sign]
        run(*command, output)
    print(f'Built: {output}')
    run('python3', ROOT / 'tools/verify-pkg.py', output)


if __name__ == '__main__':
    main()
