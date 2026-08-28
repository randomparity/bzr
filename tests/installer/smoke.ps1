# Smoke tests for install.ps1. Runs against local file:// fixtures.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$InstallPs1 = Join-Path $RepoRoot 'install.ps1'

if (-not (Test-Path $InstallPs1)) {
    Write-Error "smoke: $InstallPs1 not found"
    exit 1
}

function Build-Fixture {
    param([string]$Dir, [string]$Tag, [string]$Target)
    $staging = "bzr-$Tag-$Target"
    $stagingDir = Join-Path $Dir $staging
    New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null
    # Stub bzr.exe -- content doesn't have to be runnable when BZR_SKIP_SMOKE=1
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
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    if ($arch -eq 'AMD64') { return 'x86_64-pc-windows-msvc' }
    if ($arch -eq 'ARM64') { return 'aarch64-pc-windows-msvc' }
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
        if (-not $target) { [Console]::Error.WriteLine("smoke: skipping success_path (unsupported host)"); return }
        Build-Fixture -Dir $fixtures.FullName -Tag 'v0.0.0-test' -Target $target

        $env:BZR_BASE_URL = Convert-PathToFileUri (Join-Path $work 'releases')
        $env:BZR_VERSION = 'v0.0.0-test'
        $env:BZR_INSTALL_DIR = $installDir
        $env:BZR_INSTALL_SCRIPT = $InstallPs1
        $env:BZR_SKIP_SMOKE = '1'
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -Command @'
function Read-Host { 'N' }
& $env:BZR_INSTALL_SCRIPT
'@
            if ($LASTEXITCODE -ne 0) { throw "install.ps1 failed with exit $LASTEXITCODE" }
        } finally {
            Remove-Item Env:BZR_BASE_URL, Env:BZR_VERSION, Env:BZR_INSTALL_DIR, Env:BZR_INSTALL_SCRIPT, Env:BZR_SKIP_SMOKE -ErrorAction SilentlyContinue
        }

        if (-not (Test-Path (Join-Path $installDir 'bzr.exe'))) {
            throw "smoke: bzr.exe not installed at $installDir\bzr.exe"
        }
        [Console]::Error.WriteLine("smoke: success_path OK")
    } finally {
        Remove-Item -Recurse -Force $work
    }
}

function Test-PathPrompt {
    $work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-smoke-" + [Guid]::NewGuid())) -Force
    $originalUserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $originalProcessPath = $env:Path
    try {
        $fixtures = New-Item -ItemType Directory -Path (Join-Path $work 'releases\v0.0.0-test') -Force
        $installDir = Join-Path $work 'bin'
        $target = Get-NativeTarget
        if (-not $target) { [Console]::Error.WriteLine("smoke: skipping path_prompt (unsupported host)"); return }
        Build-Fixture -Dir $fixtures.FullName -Tag 'v0.0.0-test' -Target $target

        $env:BZR_BASE_URL = Convert-PathToFileUri (Join-Path $work 'releases')
        $env:BZR_VERSION = 'v0.0.0-test'
        $env:BZR_INSTALL_DIR = $installDir
        $env:BZR_INSTALL_SCRIPT = $InstallPs1
        $env:BZR_SKIP_SMOKE = '1'
        try {
            [Environment]::SetEnvironmentVariable('Path', $originalUserPath, 'User')
            $yesOutput = & powershell -NoProfile -ExecutionPolicy Bypass -Command @'
function Read-Host { 'Y' }
& $env:BZR_INSTALL_SCRIPT
if (($env:Path -split ';') -notcontains $env:BZR_INSTALL_DIR) {
    throw 'install directory missing from current process PATH'
}
'@ 2>&1
            if ($LASTEXITCODE -ne 0) { throw "install.ps1 Y response failed: $yesOutput" }
            $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            if (($userPath -split ';') -notcontains $installDir) {
                throw 'smoke: Y response did not update the user PATH'
            }

            [Environment]::SetEnvironmentVariable('Path', $originalUserPath, 'User')
            $noOutput = & powershell -NoProfile -ExecutionPolicy Bypass -Command @'
function Read-Host { 'N' }
& $env:BZR_INSTALL_SCRIPT
'@ 2>&1
            if ($LASTEXITCODE -ne 0) { throw "install.ps1 N response failed: $noOutput" }
            if ([Environment]::GetEnvironmentVariable('Path', 'User') -ne $originalUserPath) {
                throw 'smoke: N response changed the user PATH'
            }

            $presentUserPath = (@($originalUserPath, $installDir) | Where-Object { $_ }) -join ';'
            [Environment]::SetEnvironmentVariable('Path', $presentUserPath, 'User')
            $env:Path = (@($env:Path, $installDir) | Where-Object { $_ }) -join ';'
            $presentOutput = & powershell -NoProfile -ExecutionPolicy Bypass -Command @'
function Read-Host { throw 'installer prompted for an existing PATH entry' }
& $env:BZR_INSTALL_SCRIPT
'@ 2>&1
            if ($LASTEXITCODE -ne 0) { throw "install.ps1 existing PATH failed: $presentOutput" }
            $pathMatches = @(([Environment]::GetEnvironmentVariable('Path', 'User') -split ';') |
                Where-Object { $_ -ieq $installDir })
            if ($pathMatches.Count -ne 1) { throw "smoke: expected one user PATH entry, got $($pathMatches.Count)" }
        } finally {
            [Environment]::SetEnvironmentVariable('Path', $originalUserPath, 'User')
            $env:Path = $originalProcessPath
            Remove-Item Env:BZR_BASE_URL, Env:BZR_VERSION, Env:BZR_INSTALL_DIR, Env:BZR_INSTALL_SCRIPT, Env:BZR_SKIP_SMOKE -ErrorAction SilentlyContinue
        }

        [Console]::Error.WriteLine("smoke: path_prompt OK")
    } finally {
        [Environment]::SetEnvironmentVariable('Path', $originalUserPath, 'User')
        $env:Path = $originalProcessPath
        Remove-Item -Recurse -Force $work
    }
}

function Test-ChecksumMismatch {
    $work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-smoke-" + [Guid]::NewGuid())) -Force
    try {
        $fixtures = New-Item -ItemType Directory -Path (Join-Path $work 'releases\v0.0.0-test') -Force
        $installDir = Join-Path $work 'bin'
        $target = Get-NativeTarget
        if (-not $target) { [Console]::Error.WriteLine("smoke: skipping checksum_mismatch (unsupported host)"); return }
        Build-Fixture -Dir $fixtures.FullName -Tag 'v0.0.0-test' -Target $target
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
        [Console]::Error.WriteLine("smoke: checksum_mismatch OK")
    } finally {
        Remove-Item -Recurse -Force $work
    }
}

function Test-UnsupportedTarget {
    $work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-smoke-" + [Guid]::NewGuid())) -Force
    try {
        $installDir = Join-Path $work 'bin'

        $env:BZR_VERSION = 'v0.0.0-test'
        $env:BZR_INSTALL_DIR = $installDir
        $env:BZR_SKIP_SMOKE = '1'
        # Force install.ps1 to read this fake arch via PROCESSOR_ARCHITEW6432.
        $env:PROCESSOR_ARCHITEW6432 = 'IA64'
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -File $InstallPs1 *>$null
            $rc = $LASTEXITCODE
        } finally {
            Remove-Item Env:BZR_VERSION, Env:BZR_INSTALL_DIR, Env:BZR_SKIP_SMOKE, Env:PROCESSOR_ARCHITEW6432 -ErrorAction SilentlyContinue
        }

        if ($rc -ne 2) { throw "smoke: expected exit 2 (unsupported), got $rc" }
        [Console]::Error.WriteLine("smoke: unsupported_target OK")
    } finally {
        Remove-Item -Recurse -Force $work
    }
}

Test-SuccessPath
Test-PathPrompt
Test-ChecksumMismatch
Test-UnsupportedTarget
[Console]::Error.WriteLine("smoke: all sub-tests passed")
# Explicit clean exit: each Test-* invokes install.ps1 as a subprocess
# whose non-zero exit codes (intentionally tested) leave $LASTEXITCODE
# set. Without this, pwsh inherits the residual code at script end.
exit 0
