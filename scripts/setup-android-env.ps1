param(
  [string]$SdkRoot = "$env:LOCALAPPDATA\Android\Sdk",
  [string]$NdkVersion = "28.2.13676358",
  [string]$CommandLineToolsUrl = "https://dl.google.com/android/repository/commandlinetools-win-13114758_latest.zip",
  [switch]$SkipAndroidStudio
)

$ErrorActionPreference = "Stop"

function Add-CurrentPath {
  param([string]$PathToAdd)
  if (Test-Path -LiteralPath $PathToAdd) {
    $entries = $env:Path -split ';'
    if ($entries -notcontains $PathToAdd) {
      $env:Path = "$PathToAdd;$env:Path"
    }
  }
}

function Ensure-WingetPackage {
  param([string]$Id, [string]$Name)
  if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Host "winget is not available; skip $Name installation."
    return
  }

  Write-Host "Checking $Name..."
  $installed = winget list --id $Id -e --accept-source-agreements 2>$null
  if ($LASTEXITCODE -eq 0 -and ($installed -match [regex]::Escape($Id))) {
    Write-Host "$Name already installed."
    return
  }

  winget install --id $Id -e --accept-package-agreements --accept-source-agreements
}

function Ensure-CommandLineTools {
  $sdkManager = Join-Path $SdkRoot "cmdline-tools\latest\bin\sdkmanager.bat"
  if (Test-Path -LiteralPath $sdkManager) {
    return $sdkManager
  }

  $downloadDir = Join-Path $env:TEMP ("lilia-android-sdk-" + [guid]::NewGuid().ToString("N"))
  $zipPath = Join-Path $downloadDir "commandlinetools-win.zip"
  $extractDir = Join-Path $downloadDir "cmdline-tools"
  New-Item -ItemType Directory -Force -Path $downloadDir | Out-Null
  New-Item -ItemType Directory -Force -Path $SdkRoot | Out-Null

  Write-Host "Downloading Android command-line tools..."
  Invoke-WebRequest -Uri $CommandLineToolsUrl -OutFile $zipPath

  if (Test-Path -LiteralPath $extractDir) {
    Remove-Item -LiteralPath $extractDir -Recurse -Force
  }
  Expand-Archive -LiteralPath $zipPath -DestinationPath $extractDir -Force

  $latestDir = Join-Path $SdkRoot "cmdline-tools\latest"
  New-Item -ItemType Directory -Force -Path (Split-Path $latestDir -Parent) | Out-Null
  if (Test-Path -LiteralPath $latestDir) {
    Remove-Item -LiteralPath $latestDir -Recurse -Force
  }
  Move-Item -LiteralPath (Join-Path $extractDir "cmdline-tools") -Destination $latestDir

  return $sdkManager
}

function Ensure-AndroidPackages {
  param([string]$SdkManager)

  $packages = @(
    "platform-tools",
    "emulator",
    "platforms;android-36",
    "build-tools;36.0.0",
    "ndk;$NdkVersion",
    "system-images;android-36;google_apis;x86_64"
  )

  Write-Host "Installing Android SDK packages..."
  $yesFile = Join-Path $env:TEMP ("lilia-android-licenses-" + [guid]::NewGuid().ToString("N") + ".txt")
  1..200 | ForEach-Object { "y" } | Set-Content -LiteralPath $yesFile -Encoding ASCII
  cmd /c "type `"$yesFile`" | `"$SdkManager`" --sdk_root=`"$SdkRoot`" --licenses"
  foreach ($package in $packages) {
    & $SdkManager --sdk_root=$SdkRoot $package
  }
}

function Ensure-RustTargets {
  if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Write-Host "rustup is not available; skip Android Rust targets."
    return
  }

  rustup target add `
    aarch64-linux-android `
    armv7-linux-androideabi `
    i686-linux-android `
    x86_64-linux-android
}

if (-not $SkipAndroidStudio) {
  Ensure-WingetPackage -Id "Google.AndroidStudio" -Name "Android Studio"
}

$env:ANDROID_HOME = $SdkRoot
$env:ANDROID_SDK_ROOT = $SdkRoot
Add-CurrentPath (Join-Path $SdkRoot "platform-tools")
Add-CurrentPath (Join-Path $SdkRoot "emulator")

$sdkManager = Ensure-CommandLineTools
Ensure-AndroidPackages -SdkManager $sdkManager
Ensure-RustTargets

$localProperties = Join-Path (Resolve-Path ".") "apps\android\local.properties"
$sdkDir = $SdkRoot.Replace('\', '/')
"sdk.dir=$sdkDir" | Set-Content -LiteralPath $localProperties -Encoding ASCII

Write-Host ""
Write-Host "Android SDK setup attempted."
Write-Host "SDK root: $SdkRoot"
Write-Host "Run: yarn android:doctor"
