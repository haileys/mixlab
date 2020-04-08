#requires -version 3
<#
.SYNOPSIS
  Windows build script for mixlab
.PARAMETER Build
    Builds project
.PARAMETER Check
    Checks project
.PARAMETER Run
    Runs project
.PARAMETER Release
    Builds/runs/checks in release mode
.EXAMPLE
  Build-Frontend -Build -Release
#>

Param(
  [Parameter(ParameterSetName="build")][ValidateScript({$_})][switch]$Build,
  [Parameter(Mandatory,ParameterSetName="check")][ValidateScript({$_})][switch]$Check,
  [Parameter(Mandatory,ParameterSetName="run")][ValidateScript({$_})][switch]$Run,
  [Parameter(HelpMessage="Build in release mode")][switch]$Release
)

if (-not ($Check -or $Build -or $Run)) {
  $Build = $True
}

if ($null -eq (Get-Command "wasm-pack.exe" -ErrorAction SilentlyContinue))
{
  Write-Host "wasm-pack is not installed... please install it from https://rustwasm.github.io/wasm-pack/installer/"
  exit 1
}
Function Enter-Frontend {
  try {
    Push-Location -Path (Join-Path -Path (Get-Location) -ChildPath "frontend")
    $orig_RUSTFLAGS = $env:RUSTFLAGS
    $env:RUSTFLAGS = "$orig_RUSTFLAGS --remap-path-prefix src=frontend/src"
    $rest = $args[1..($args.Length - 1)]
    & $args[0] @rest
  } finally {
    $env:RUSTFLAGS = $orig_RUSTFLAGS
    Pop-Location
  }
}

if ($Release) {
  $BuildMode = "--release"
  $WasmBuildMode = "--release"
} else {
  $WasmBuildMode = "--dev"
}

if ($Check) {
  Enter-Frontend cargo check $BuildMode --target=wasm32-unknown-unknown
  if ($LastExitCode -eq 0) { cargo check $BuildMode }
} else {
  Enter-Frontend wasm-pack.exe build $WasmBuildMode "--target" no-modules
  if ($LastExitCode -eq 0) {
    Write-Host test
    if ($Build) {
      & cargo build $BuildMode
    } elseif ($Run) {
      & cargo run $BuildMode
    }
  }
}

exit $LastExitCode