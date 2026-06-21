param(
  [string]$IrohFfiDir = $env:IROH_FFI_DIR,
  [string]$Abi = "arm64-v8a"
)

$ErrorActionPreference = "Stop"

if (-not $IrohFfiDir) {
  throw "Set IROH_FFI_DIR to a local iroh-ffi checkout before building Android bindings."
}

$resolved = Resolve-Path -LiteralPath $IrohFfiDir
$androidDir = Join-Path $resolved "iroh-android"
if (-not (Test-Path -LiteralPath $androidDir)) {
  throw "Cannot find iroh Android binding project at $androidDir"
}

Write-Host "Using iroh-ffi: $resolved"
Write-Host "Target ABI: $Abi"
Write-Host "Run the iroh-ffi Android build from its source tree, then publish or copy the AAR into apps/android/app/libs."
Write-Host "This wrapper intentionally keeps third-party source outside the Lilia repo."
