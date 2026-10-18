# Run from the extracted KekLocator Windows package. No administrator rights needed.
$ErrorActionPreference = 'Stop'
$taskPackage = $PSScriptRoot
$taskDestinations = @{ 'KekLocator.vst3' = "$env:LOCALAPPDATA/Programs/Common/VST3"; 'KekLocator.clap' = "$env:LOCALAPPDATA/Programs/Common/CLAP" }
foreach ($taskName in $taskDestinations.Keys) {
    $taskSource = Join-Path $taskPackage $taskName
    if (-not (Test-Path $taskSource)) { throw "Missing plug-in: $taskName. Extract the whole ZIP first." }
    New-Item -ItemType Directory -Force $taskDestinations[$taskName] | Out-Null
    Copy-Item $taskSource $taskDestinations[$taskName] -Recurse -Force
    Write-Host "Installed $taskName in $($taskDestinations[$taskName])"
}
Write-Host 'Rescan plug-ins in your DAW. Add the paths above to its search paths if needed.'
