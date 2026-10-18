$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Set-Location $taskRoot
function Run-Checked([scriptblock]$Command) { & $Command; if ($LASTEXITCODE -ne 0) { throw "Command failed: $Command" } }
Run-Checked { python tools/prepare-firmware.py }
Run-Checked { python tools/prepare-plugin-sdk.py }
$env:RUSTC = (& rustup which --toolchain stable rustc).Trim()
$env:RUSTDOC = (& rustup which --toolchain stable rustdoc).Trim()
Run-Checked { rustup target add --toolchain stable wasm32-unknown-unknown x86_64-pc-windows-msvc }
$bindgen = Join-Path $taskRoot '.tools/bin/wasm-bindgen.exe'
if (-not (Test-Path $bindgen)) { Run-Checked { rustup run stable cargo install wasm-bindgen-cli --version 0.2.129 --locked --root .tools } }
$env:RUSTFLAGS = ''
Run-Checked { rustup run stable cargo build --locked --release --target wasm32-unknown-unknown -p keklocator-ui }
New-Item -ItemType Directory -Force dist | Out-Null
Run-Checked { & $bindgen --target no-modules --out-dir dist --out-name plugin-ui target/wasm32-unknown-unknown/release/keklocator_ui.wasm }
Copy-Item web/plugin* dist -Force
Run-Checked { python tools/package-plugin-ui.py }
$env:RUSTFLAGS = '-C target-feature=+crt-static'
Run-Checked { rustup run stable cargo build --locked --release --target x86_64-pc-windows-msvc -p keklocator-plugin }
$taskLibrary = Join-Path $taskRoot 'target/x86_64-pc-windows-msvc/release/keklocator_plugin.lib'
Run-Checked { cmake -S plugins -B target/plugin-build-win -A x64 "-DECHO_RUST_LIBRARY=$taskLibrary" }
Run-Checked { cmake --build target/plugin-build-win --config Release --parallel 4 }
$taskPackage = Join-Path $taskRoot 'build/plugins/windows-x64'
New-Item -ItemType Directory -Force $taskPackage | Out-Null
if (-not (Test-Path 'target/plugin-build-win/out/KekLocator.vst3/Contents/x86_64-win/KekLocator.vst3')) { throw 'VST3 binary is missing' }
if (-not (Test-Path 'target/plugin-build-win/out/KekLocator.clap')) { throw 'CLAP binary is missing' }
Copy-Item target/plugin-build-win/out/KekLocator.vst3 $taskPackage -Recurse -Force
Copy-Item target/plugin-build-win/out/KekLocator.clap $taskPackage -Force
Copy-Item tools/install-windows.ps1 "$taskPackage/Install.ps1" -Force
Copy-Item plugins/windows-README.txt "$taskPackage/README.txt" -Force
Copy-Item NOTICE.md "$taskPackage/NOTICE.md" -Force
Compress-Archive -Path "$taskPackage/*" -DestinationPath build/plugins/KekLocator-1.2.0-Windows-x64.zip -Force
Get-FileHash build/plugins/KekLocator-1.2.0-Windows-x64.zip -Algorithm SHA256
