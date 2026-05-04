# Smoke tests for install.ps1. Runs against local file:// fixtures.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$InstallPs1 = Join-Path $RepoRoot 'install.ps1'

if (-not (Test-Path $InstallPs1)) {
    Write-Error "smoke: $InstallPs1 not found"
    exit 1
}

function New-Fixtures {
    param([string]$Dir, [string]$Tag, [string]$Target)
    $staging = "bzr-$Tag-$Target"
    $stagingDir = Join-Path $Dir $staging
    New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null
    # Stub bzr.exe — content doesn't have to be runnable when BZR_SKIP_SMOKE=1
    Set-Content -Path (Join-Path $stagingDir 'bzr.exe') -Value 'fake bzr binary' -NoNewline
    Set-Content -Path (Join-Path $stagingDir 'LICENSE') -Value 'fake LICENSE' -NoNewline
    Set-Content -Path (Join-Path $stagingDir 'README.md') -Value 'fake README' -NoNewline
    Compress-Archive -Path $stagingDir -DestinationPath (Join-Path $Dir "$staging.zip")
    Remove-Item -Recurse -Force $stagingDir
    # Generate SHA256SUMS
    Push-Location $Dir
    try {
        $hash = (Get-FileHash -Algorithm SHA256 "$staging.zip").Hash.ToLower()
        Set-Content -Path 'SHA256SUMS' -Value "$hash  $staging.zip" -Encoding ascii -NoNewline
    } finally { Pop-Location }
}

function Get-NativeTarget {
    if ($env:PROCESSOR_ARCHITECTURE -eq 'AMD64') { return 'x86_64-pc-windows-msvc' }
    if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { return 'aarch64-pc-windows-msvc' }
    return ''
}

function Convert-PathToFileUri {
    param([string]$Path)
    # Convert a Windows local path to a file:// URI install.ps1 can fetch.
    $abs = (Resolve-Path $Path).Path
    "file:///" + ($abs -replace '\\', '/')
}

function Test-SuccessPath {
    $work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-smoke-" + [Guid]::NewGuid())) -Force
    try {
        $fixtures = New-Item -ItemType Directory -Path (Join-Path $work 'releases\v0.0.0-test') -Force
        $installDir = Join-Path $work 'bin'
        $target = Get-NativeTarget
        if (-not $target) { Write-Host "smoke: skipping success_path (unsupported host)"; return }
        New-Fixtures -Dir $fixtures.FullName -Tag 'v0.0.0-test' -Target $target

        $env:BZR_BASE_URL = Convert-PathToFileUri (Join-Path $work 'releases')
        $env:BZR_VERSION = 'v0.0.0-test'
        $env:BZR_INSTALL_DIR = $installDir
        $env:BZR_SKIP_SMOKE = '1'
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -File $InstallPs1
            if ($LASTEXITCODE -ne 0) { throw "install.ps1 failed with exit $LASTEXITCODE" }
        } finally {
            Remove-Item Env:BZR_BASE_URL, Env:BZR_VERSION, Env:BZR_INSTALL_DIR, Env:BZR_SKIP_SMOKE -ErrorAction SilentlyContinue
        }

        if (-not (Test-Path (Join-Path $installDir 'bzr.exe'))) {
            throw "smoke: bzr.exe not installed at $installDir\bzr.exe"
        }
        Write-Host "smoke: success_path OK"
    } finally {
        Remove-Item -Recurse -Force $work
    }
}

function Test-ChecksumMismatch {
    $work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-smoke-" + [Guid]::NewGuid())) -Force
    try {
        $fixtures = New-Item -ItemType Directory -Path (Join-Path $work 'releases\v0.0.0-test') -Force
        $installDir = Join-Path $work 'bin'
        $target = Get-NativeTarget
        if (-not $target) { Write-Host "smoke: skipping checksum_mismatch (unsupported host)"; return }
        New-Fixtures -Dir $fixtures.FullName -Tag 'v0.0.0-test' -Target $target
        # Corrupt the archive after sums were generated.
        Add-Content -Path (Join-Path $fixtures.FullName "bzr-v0.0.0-test-$target.zip") -Value 'tampered'

        $env:BZR_BASE_URL = Convert-PathToFileUri (Join-Path $work 'releases')
        $env:BZR_VERSION = 'v0.0.0-test'
        $env:BZR_INSTALL_DIR = $installDir
        $env:BZR_SKIP_SMOKE = '1'
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -File $InstallPs1 *>$null
            $rc = $LASTEXITCODE
        } finally {
            Remove-Item Env:BZR_BASE_URL, Env:BZR_VERSION, Env:BZR_INSTALL_DIR, Env:BZR_SKIP_SMOKE -ErrorAction SilentlyContinue
        }

        if ($rc -ne 5) { throw "smoke: expected exit 5 (checksum mismatch), got $rc" }
        if (Test-Path (Join-Path $installDir 'bzr.exe')) {
            throw "smoke: bzr.exe should NOT be installed when checksum fails"
        }
        Write-Host "smoke: checksum_mismatch OK"
    } finally {
        Remove-Item -Recurse -Force $work
    }
}

Test-SuccessPath
Test-ChecksumMismatch
Write-Host "smoke: all sub-tests passed"
