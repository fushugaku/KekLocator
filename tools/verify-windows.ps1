$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Set-Location $taskRoot
function Run-Checked([scriptblock]$Command) { & $Command; if ($LASTEXITCODE -ne 0) { throw "Command failed: $Command" } }
Run-Checked { cmake -S tools/windows-tests -B target/windows-tests -A x64 }
Run-Checked { cmake --build target/windows-tests --config Release --target kek-clap-test kek-vst3-test validator --parallel 4 }
$env:KEKLOCATOR_TEST_LIBRARY = '1'
$env:APPDATA = Join-Path $taskRoot 'target/windows-library-test'
$taskPackage = Join-Path $taskRoot 'build/plugins/windows-x64'
Run-Checked { & target/windows-tests/Release/kek-clap-test.exe "$taskPackage/KekLocator.clap" }
Run-Checked { & target/windows-tests/Release/kek-vst3-test.exe "$taskPackage/KekLocator.vst3/Contents/x86_64-win/KekLocator.vst3" }
$taskValidator = Get-ChildItem target/windows-tests -Recurse -Filter validator.exe | Select-Object -First 1
if (-not $taskValidator) { throw 'The official VST3 validator was not built' }
Run-Checked { & $taskValidator.FullName "$taskPackage/KekLocator.vst3" }
@{ product='KekLocator'; version='1.2.0'; platform='Windows x64'; status='PASS'; native_render_cases=16; rates=@(44100,48000,96000,192000); modes=@('Original','Extended'); checks=@('CLAP and VST3 audio versus Rust','tempo and transport','seek and loop','stop','variable block sizes','latency','timestamped automation','physical parameter parsing','state round trip','legacy 64-parameter state','official Steinberg VST3 validator','embedded editor resource in both binaries','Windows user-library write and replacement') } | ConvertTo-Json -Depth 4 | Set-Content build/plugins/windows-verification.json -Encoding utf8
