#Requires -Version 5
# bzr-skill installer (PowerShell). Copies skill folders into agent skill directories.
#
# PARITY GAP: Unlike install.sh, this script does NOT take a mkdir-based concurrency lock.
# Windows installs are assumed to be single-user/interactive, so concurrent runs are not
# expected. The POSIX version uses mkdir-based locking to guard against races. If this
# script is ever invoked from automation or CI that may run it in parallel, add a
# mutex-based lock here.
[CmdletBinding()]
param(
    [ValidateSet('standard', 'bob', 'codex', 'claude', 'all')]
    [string]$Agent,
    [switch]$DryRun,
    [switch]$Force,
    [switch]$Uninstall,
    [switch]$List,
    [switch]$Help
)

$ErrorActionPreference = 'Stop'
$script:Failed = $false
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$SkillsSrc = Join-Path $ScriptDir 'skills'
$VersionFile = Join-Path $ScriptDir 'VERSION'
$Sentinel = '.bzr-skill-managed'
$SkillNames = @('bzr-reference', 'bzr-setup', 'bzr-file-bug', 'bzr-triage-bug', 'bzr-search-report', 'bzr-bulk-triage')
$DestRoot = if ($env:BZR_SKILL_DEST_ROOT) { $env:BZR_SKILL_DEST_ROOT } else { $HOME }
$AgentsDir = Join-Path $DestRoot '.agents/skills'
$ClaudeDir = Join-Path $DestRoot '.claude/skills'

function Show-Usage {
    @"
bzr-skill installer (PowerShell)

Usage: install.ps1 -Agent <standard|bob|codex|claude|all> [-DryRun] [-Force] [-Uninstall] [-List]

Targets (-Agent):
  standard | bob | codex  -> $AgentsDir
  claude                  -> $ClaudeDir
  all                     -> both

Options:
  -DryRun    show the plan without writing
  -Force     overwrite a foreign same-named folder (never overrides symlink guard)
  -Uninstall remove skill folders this installer owns
  -List      show which bzr skills are installed where (and stale ones)
  -Help      this message

Env:
  BZR_SKILL_DEST_ROOT   destination root (default: `$HOME); used by tests.
"@ | Write-Output
}

function Get-Destinations {
    param([string]$AgentName)
    switch ($AgentName) {
        { $_ -in 'standard', 'bob', 'codex' } { return @($AgentsDir) }
        'claude' { return @($ClaudeDir) }
        'all' { return @($AgentsDir, $ClaudeDir) }
        default { throw "unknown agent: $AgentName" }
    }
}

function Get-SourceVersion {
    if (Test-Path $VersionFile) {
        (Get-Content $VersionFile -First 1).Trim()
    } else {
        'unknown'
    }
}

function Get-SourceCommit {
    $commit = (& git -C $ScriptDir rev-parse --short HEAD 2>$null)
    if (-not $commit) { $commit = 'unknown' }
    return $commit
}

function Test-Owned {
    param([string]$Folder)
    Test-Path (Join-Path $Folder $Sentinel)
}

function Test-IsSymlink {
    param([string]$Path)
    $item = Get-Item $Path -Force -ErrorAction SilentlyContinue
    if (-not $item) { return $false }
    return [bool]($item.Attributes -band [IO.FileAttributes]::ReparsePoint)
}

# SECURITY GUARD: Refuse if the destination dir or any ancestor down from DestRoot is a
# symlink/reparse-point. This prevents home-directory-escape attacks where an attacker
# plants a symlink in the path to redirect writes outside the intended tree.
# Mirrors reject_symlink_path in install.sh. -Force does NOT bypass this check.
function Assert-NoSymlinkPath {
    param([string]$Dir)
    $d = $Dir
    $root = [IO.Path]::GetPathRoot($d)
    while ($d -and $d -ne $DestRoot -and $d -ne $root) {
        if (Test-IsSymlink $d) {
            throw "refusing ${Dir}: ancestor '$d' is a symlink/reparse-point (escape guard)"
        }
        $d = Split-Path -Parent $d
    }
    # Also check the directory itself (leaf guard)
    if (Test-IsSymlink $Dir) {
        throw "refusing ${Dir}: destination itself is a symlink/reparse-point (escape guard)"
    }
}

function Write-Sentinel {
    param([string]$Folder, [string]$Skill)
    @(
        'managed-by: bzr-skill',
        "installed-skill: $Skill",
        "source-version: $(Get-SourceVersion)",
        "source-commit: $(Get-SourceCommit)"
    ) | Set-Content -Path (Join-Path $Folder $Sentinel) -Encoding UTF8
}

function Write-InstallError {
    # Write-Error with $ErrorActionPreference='Stop' throws immediately, bypassing
    # 'return $false'. Use this wrapper to emit to stderr without throwing.
    param([string]$Message)
    [Console]::Error.WriteLine("install: ERROR $Message")
}

function Install-Skill {
    param([string]$Skill, [string]$Dest)

    $src = Join-Path $SkillsSrc $Skill
    $target = Join-Path $Dest $Skill

    if (-not (Test-Path $src -PathType Container)) {
        Write-InstallError "source skill missing: $src"
        return $false
    }

    # Leaf symlink guard — unconditional, -Force does NOT bypass.
    if (Test-IsSymlink $target) {
        Write-InstallError "REFUSE ${target}: destination is a symlink/reparse-point (unconditional guard)"
        return $false
    }

    # Foreign folder guard — bypassed by -Force.
    if ((Test-Path $target) -and -not (Test-Owned $target) -and -not $Force) {
        Write-InstallError "REFUSE ${target}: foreign folder (no sentinel); use -Force to overwrite"
        return $false
    }

    $stage = Join-Path $Dest (".bzr-skill.stage.$Skill.$PID")
    if (Test-Path $stage) {
        Remove-Item $stage -Recurse -Force
    }

    Copy-Item $src $stage -Recurse
    Write-Sentinel $stage $Skill

    # Verify staged copy BEFORE touching original.
    if (-not (Test-Path (Join-Path $stage 'SKILL.md'))) {
        Remove-Item $stage -Recurse -Force
        Write-InstallError "REFUSE ${Skill}: staged copy missing SKILL.md; aborting (original untouched)"
        return $false
    }

    if (Test-Path $target) {
        $aside = Join-Path $Dest (".bzr-skill.old.$Skill.$PID")
        if (Test-Path $aside) { Remove-Item $aside -Recurse -Force }
        Move-Item $target $aside
        try {
            Move-Item $stage $target
            Remove-Item $aside -Recurse -Force
        } catch {
            # Restore original if the swap fails.
            Move-Item $aside $target
            if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
            Write-InstallError "ERROR replacing ${target}; restored original: $_"
            return $false
        }
    } else {
        Move-Item $stage $target
    }

    return (Test-Path (Join-Path $target 'SKILL.md'))
}

function Invoke-Install {
    param([string[]]$Dests)

    if (-not (Get-Command bzr -ErrorAction SilentlyContinue)) {
        Write-Warning 'bzr not found on PATH; install it from https://github.com/randomparity/bzr'
    }

    foreach ($dest in $Dests) {
        Assert-NoSymlinkPath $dest

        if ($DryRun) {
            Write-Output "plan: install into $dest"
            $SkillNames | ForEach-Object { Write-Output "  - $_" }
            continue
        }

        if (-not (Test-Path $dest)) {
            New-Item -ItemType Directory -Path $dest -Force | Out-Null
        }

        foreach ($s in $SkillNames) {
            if (Install-Skill $s $dest) {
                Write-Output "install: ok   $s -> $dest"
            } else {
                Write-Output "install: failed $s -> $dest"
                $script:Failed = $true
            }
        }
    }
}

function Invoke-Uninstall {
    param([string[]]$Dests)

    foreach ($dest in $Dests) {
        Assert-NoSymlinkPath $dest

        if (-not (Test-Path $dest)) { continue }

        foreach ($s in $SkillNames) {
            $target = Join-Path $dest $s

            if (Test-IsSymlink $target) {
                Write-Output "uninstall: skip $target (symlink, not touching)"
                continue
            }

            if (-not (Test-Path $target)) { continue }

            if (Test-Owned $target) {
                $aside = Join-Path $dest (".bzr-skill.rm.$s.$PID")
                try {
                    Move-Item $target $aside
                    Remove-Item $aside -Recurse -Force
                    Write-Output "uninstall: removed $target"
                } catch {
                    [Console]::Error.WriteLine("uninstall: ERROR removing ${target}: $_")
                    $script:Failed = $true
                }
            } else {
                Write-Output "uninstall: skip $target (foreign folder)"
            }
        }
    }
}

function Get-SentinelVersion {
    param([string]$Folder)
    $sentinelPath = Join-Path $Folder $Sentinel
    if (-not (Test-Path $sentinelPath)) { return '' }
    $match = Select-String -Path $sentinelPath -Pattern '^source-version:\s*(.+)$'
    if ($match) { return $match.Matches[0].Groups[1].Value.Trim() }
    return ''
}

function Invoke-List {
    param([string[]]$Dests)

    $cur = Get-SourceVersion

    foreach ($dest in $Dests) {
        Assert-NoSymlinkPath $dest
        Write-Output "destination: $dest"

        foreach ($s in $SkillNames) {
            $target = Join-Path $dest $s

            if (Test-IsSymlink $target) {
                Write-Output ("  {0,-18} symlink (shadowed)" -f $s)
            } elseif (-not (Test-Path $target)) {
                Write-Output ("  {0,-18} absent" -f $s)
            } elseif (Test-Owned $target) {
                $iv = Get-SentinelVersion $target
                if ($iv -eq $cur) {
                    Write-Output ("  {0,-18} present ({1})" -f $s, $iv)
                } else {
                    Write-Output ("  {0,-18} present, stale (installed {1}, source {2}) -- re-run install.ps1" -f $s, $iv, $cur)
                }
            } else {
                Write-Output ("  {0,-18} shadowed (foreign folder)" -f $s)
            }
        }
    }
}

# Entry point
if ($Help -or (-not $Agent -and -not $List)) {
    Show-Usage
    return
}

$targets = if ($List -and -not $Agent) {
    Get-Destinations 'all'
} else {
    Get-Destinations $Agent
}

if ($Uninstall) {
    Invoke-Uninstall $targets
} elseif ($List) {
    Invoke-List $targets
} else {
    Invoke-Install $targets
}

if ($script:Failed) { exit 1 }
