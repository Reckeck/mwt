# Windows release build on a GitLab Windows runner: NSIS installer (.exe),
# MSI (one per language) and a portable zip with the bare .exe.
#
#   pwsh scripts/ci/build-windows.ps1 -OutDir release
#
# Rust/Node/pnpm versions come from mise.toml (mise itself is not used here).
param([string]$OutDir = 'release')

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

function Invoke-Native {
  param([Parameter(ValueFromRemainingArguments)] $Command)
  & $Command[0] @($Command | Select-Object -Skip 1)
  if ($LASTEXITCODE -ne 0) { throw "failed ($LASTEXITCODE): $($Command -join ' ')" }
}

$root = (Get-Location).Path
$cache = Join-Path $root '.cache'
New-Item -ItemType Directory -Force $cache | Out-Null

$mise = Get-Content mise.toml -Raw
function Get-Tool([string]$name) { [regex]::Match($mise, "(?m)^$name = `"([^`"]+)`"").Groups[1].Value }
$rustVersion = Get-Tool 'rust'
$nodeVersion = Get-Tool 'node'
$pnpmVersion = Get-Tool 'pnpm'
# The version this build ships under: BUILD_VERSION from the version job
# (scripts/ci/version.sh), else the plain app version for a local run.
$version = $env:BUILD_VERSION
if (-not $version) { $version = (Get-Content crates/app/tauri.conf.json -Raw | ConvertFrom-Json).version }
$versionCode = $env:ANDROID_VERSION_CODE
if (-not $versionCode) { $versionCode = '1' }
Write-Host "Multi Wallet Tracker $version - rust $rustVersion, node $nodeVersion, pnpm $pnpmVersion"

# MSVC linker: present on the GitLab image; install the build tools otherwise.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasVc = (Test-Path $vswhere) -and (& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
if (-not $hasVc) {
  Invoke-Native choco install -y --no-progress visualstudio2022buildtools `
    --package-parameters '--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
}

# Rust
$env:CARGO_HOME = Join-Path $cache 'cargo'
$env:RUSTUP_HOME = Join-Path $cache 'rustup'
$env:Path = "$env:CARGO_HOME\bin;$env:Path"
if (-not (Test-Path "$env:CARGO_HOME\bin\rustup.exe")) {
  Invoke-WebRequest https://win.rustup.rs/x86_64 -OutFile "$cache\rustup-init.exe"
  Invoke-Native "$cache\rustup-init.exe" -y --profile minimal --default-toolchain $rustVersion --no-modify-path
}
Invoke-Native rustup toolchain install $rustVersion --profile minimal
Invoke-Native rustup default $rustVersion

# sccache: compiled crates come back from the GitLab cache (.cache\sccache).
$sccacheVersion = '0.10.0'
$sccache = Join-Path $cache "sccache-$sccacheVersion.exe"
if (-not (Test-Path $sccache)) {
  $name = "sccache-v$sccacheVersion-x86_64-pc-windows-msvc"
  Invoke-WebRequest "https://github.com/mozilla/sccache/releases/download/v$sccacheVersion/$name.zip" -OutFile "$cache\sccache.zip"
  Expand-Archive -Force "$cache\sccache.zip" -DestinationPath $cache
  Move-Item "$cache\$name\sccache.exe" $sccache
}
$env:RUSTC_WRAPPER = $sccache
$env:SCCACHE_DIR = Join-Path $cache 'sccache'
$env:SCCACHE_CACHE_SIZE = '3G'
$env:CARGO_INCREMENTAL = '0'

# Node + pnpm
$nodeDir = Join-Path $cache "node-v$nodeVersion-win-x64"
if (-not (Test-Path $nodeDir)) {
  Invoke-WebRequest "https://nodejs.org/dist/v$nodeVersion/node-v$nodeVersion-win-x64.zip" -OutFile "$cache\node.zip"
  Expand-Archive "$cache\node.zip" -DestinationPath $cache
}
$env:Path = "$nodeDir;$env:Path"
Invoke-Native npm install -g "pnpm@$pnpmVersion"
# pnpm straight through node: the npm shims (pnpm.ps1 / pnpm.cmd) handed it
# the whole argument list as one string on the GitLab Windows runner.
$pnpmCli = Join-Path $nodeDir 'node_modules\pnpm\bin\pnpm.cjs'
if (-not (Test-Path $pnpmCli)) { throw "pnpm not found at $pnpmCli" }
function Invoke-Pnpm { Invoke-Native node $pnpmCli @args }
# The store goes in the GitLab cache; pnpm reads npm_config_* settings itself.
$env:npm_config_store_dir = Join-Path $cache 'pnpm'
Invoke-Pnpm --version
Invoke-Pnpm install --frozen-lockfile --prefer-offline

# Build (NSIS/WiX are fetched by the Tauri bundler)
$env:CARGO_TARGET_DIR = Join-Path $root '.rust-target'
# The version goes in as a config file: inline JSON loses its quotes on the
# way to a native command in Windows PowerShell 5.1.
$tauriConfig = Join-Path $cache 'tauri-ci.json'
$overlay = '{"version":"' + $version + '","bundle":{"android":{"versionCode":' + $versionCode + '}}}'
[System.IO.File]::WriteAllText($tauriConfig, $overlay)
# Quoted: a bare a,b in a PowerShell command line is an array, and tauri
# would get 'nsis msi'.
Invoke-Pnpm tauri build --ci --bundles 'nsis,msi' --config $tauriConfig

# Collect under stable names
$bundle = Join-Path $env:CARGO_TARGET_DIR 'release\bundle'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$setup = Get-ChildItem "$bundle\nsis\*-setup.exe" | Select-Object -First 1
if (-not $setup) { throw 'NSIS installer not found' }
Copy-Item $setup.FullName "$OutDir\multi-wallet-tracker-$version-x64-setup.exe"
$msis = Get-ChildItem "$bundle\msi\*.msi"
if (-not $msis) { throw 'MSI not found' }
foreach ($msi in $msis) {
  $lang = [regex]::Match($msi.Name, '_([a-z]{2}-[A-Z]{2})\.msi$').Groups[1].Value
  Copy-Item $msi.FullName "$OutDir\multi-wallet-tracker-$version-x64-$lang.msi"
}

# Portable: the bare exe (needs the WebView2 runtime, built into Windows 10/11)
$portable = Join-Path $cache "multi-wallet-tracker-$version-windows-x64-portable"
New-Item -ItemType Directory -Force $portable | Out-Null
Copy-Item (Join-Path $env:CARGO_TARGET_DIR 'release\multi-wallet-tracker.exe') "$portable\multi-wallet-tracker.exe"
Compress-Archive -Force -Path "$portable\*" -DestinationPath "$OutDir\multi-wallet-tracker-$version-windows-x64-portable.zip"

Get-ChildItem $OutDir | Format-Table Name, Length
& $sccache --show-stats
