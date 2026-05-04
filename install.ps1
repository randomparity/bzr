<#
.SYNOPSIS
  bzr installer for Windows.

.DESCRIPTION
  Usage:
    irm https://raw.githubusercontent.com/randomparity/bzr/main/install.ps1 | iex

  Env vars:
    $env:BZR_VERSION       - release tag (default: latest stable)
    $env:BZR_INSTALL_DIR   - install directory (default: %LOCALAPPDATA%\Programs\bzr)
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Force TLS 1.2 -- PS 5.1 defaults to SSL3/TLS1.0 which fails against GitHub.
[Net.ServicePointManager]::SecurityProtocol =
    [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

# RELEASE_VERSION_PIN -- release.yml rewrites the next line at release time
$BzrVersion = $env:BZR_VERSION
$BzrInstallDir = if ($env:BZR_INSTALL_DIR) { $env:BZR_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\bzr' }

# Internal: undocumented test override for the GitHub releases base URL.
$BaseUrl = if ($env:BZR_BASE_URL) { $env:BZR_BASE_URL } else { 'https://github.com/randomparity/bzr/releases/download' }
# Internal: undocumented test flag to skip the post-install --version call.
$SkipSmoke = [bool]$env:BZR_SKIP_SMOKE

$GithubApi = 'https://api.github.com/repos/randomparity/bzr/releases/latest'

function Write-Err($msg) { [Console]::Error.WriteLine("install.ps1: $msg") }

function Get-ResolvedArch {
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    return $arch
}

function Get-Target {
    switch (Get-ResolvedArch) {
        'AMD64' { return 'x86_64-pc-windows-msvc' }
        'ARM64' { return 'aarch64-pc-windows-msvc' }
        default { return $null }
    }
}

function Resolve-Version {
    if ($BzrVersion) { return $BzrVersion }
    try {
        $resp = Invoke-WebRequest -UseBasicParsing -Uri $GithubApi
        $obj = $resp.Content | ConvertFrom-Json
        if (-not $obj.tag_name) { throw "tag_name missing in API response" }
        return $obj.tag_name
    } catch {
        Write-Err "failed to query GitHub API for the latest release"
        Write-Err "set `$env:BZR_VERSION = 'vX.Y.Z' to pin to a specific tag"
        exit 4
    }
}

function Get-ExpectedHash {
    param([string]$SumsPath, [string]$Filename)
    $line = Get-Content $SumsPath | Where-Object { $_ -match "  $([regex]::Escape($Filename))$" } | Select-Object -First 1
    if (-not $line) {
        Write-Err "checksum line not found for $Filename in SHA256SUMS"
        exit 5
    }
    return ($line -split '\s+')[0].ToLower()
}

# --- main ---
$target = Get-Target
if (-not $target) {
    Write-Err "unsupported platform: $(Get-ResolvedArch)"
    Write-Err "Try one of:"
    Write-Err "  - cargo install bzr --locked"
    Write-Err "  - Homebrew (on macOS/Linux): brew tap randomparity/tap && brew install bzr"
    exit 2
}

$tag = Resolve-Version
$archive = "bzr-$tag-$target.zip"
$archiveUrl = "$BaseUrl/$tag/$archive"
$sumsUrl = "$BaseUrl/$tag/SHA256SUMS"

$work = New-Item -ItemType Directory -Path (Join-Path $env:TEMP ("bzr-install-" + [Guid]::NewGuid())) -Force
try {
    [Console]::Error.WriteLine("install.ps1: downloading $archiveUrl")
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $archiveUrl -OutFile (Join-Path $work $archive)
        Invoke-WebRequest -UseBasicParsing -Uri $sumsUrl    -OutFile (Join-Path $work 'SHA256SUMS')
    } catch {
        Write-Err "download failed: $_"
        exit 4
    }

    $expected = Get-ExpectedHash -SumsPath (Join-Path $work 'SHA256SUMS') -Filename $archive
    $actual = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $work $archive)).Hash.ToLower()
    if ($expected -ne $actual) {
        Write-Err "SHA-256 verification failed: expected $expected, got $actual"
        exit 5
    }

    try {
        Expand-Archive -Path (Join-Path $work $archive) -DestinationPath $work -Force
    } catch {
        Write-Err "extraction failed: $_"
        exit 6
    }

    if (-not (Test-Path $BzrInstallDir)) {
        New-Item -ItemType Directory -Path $BzrInstallDir -Force | Out-Null
    }
    Copy-Item -Path (Join-Path $work "bzr-$tag-$target\bzr.exe") -Destination (Join-Path $BzrInstallDir 'bzr.exe') -Force

    if (-not $SkipSmoke) {
        try {
            & (Join-Path $BzrInstallDir 'bzr.exe') --version | Out-Null
        } catch {
            Write-Err "bzr installed at $BzrInstallDir\bzr.exe but failed to run: $_"
        }
    }

    [Console]::Error.WriteLine("install.ps1: installed bzr to $BzrInstallDir\bzr.exe")

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $procPath = $env:Path
    if (($userPath -notlike "*$BzrInstallDir*") -and ($procPath -notlike "*$BzrInstallDir*")) {
        [Console]::Error.WriteLine(@"
install.ps1: $BzrInstallDir is not on your PATH. Add it (current user, persistent):
  [Environment]::SetEnvironmentVariable('Path',
    [Environment]::GetEnvironmentVariable('Path','User') + ';$BzrInstallDir',
    'User')
"@)
    }
} finally {
    Remove-Item -Recurse -Force $work
}
