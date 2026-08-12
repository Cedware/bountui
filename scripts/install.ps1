# Install the latest bountui release from GitHub.
#
# Usage (PowerShell):
#   irm https://raw.githubusercontent.com/Cedware/bountui/main/scripts/install.ps1 | iex
#
# The install directory can be customized:
#   $env:BOUNTUI_INSTALL_DIR = "C:\tools\bin"
#   irm https://raw.githubusercontent.com/Cedware/bountui/main/scripts/install.ps1 | iex
$ErrorActionPreference = 'Stop'

$Repo = "Cedware/bountui"
$InstallDir = if ($env:BOUNTUI_INSTALL_DIR) { $env:BOUNTUI_INSTALL_DIR } else { Join-Path $HOME ".local\bin" }

function Fail([string]$Message) {
    # Throw instead of exit so an `irm ... | iex` session is not killed on error.
    throw "error: $Message"
}

function Get-Target() {
    switch ($env:PROCESSOR_ARCHITECTURE) {
        "AMD64" { return "x86_64-pc-windows-gnu" }
        default { Fail "unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
    }
}

# GitHub requires TLS 1.2 (Windows PowerShell 5.1 defaults to TLS 1.0).
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Target = Get-Target

Write-Host "Fetching latest bountui release..."
$Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "bountui-installer" }
$Version = $Release.tag_name -replace '^v', ''
if (-not $Version) { Fail "failed to determine the latest release version" }

$Asset = "bountui-$Version-$Target.zip"
$DownloadUrl = "https://github.com/$Repo/releases/download/v$Version/$Asset"

$TmpDir = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $TmpDir | Out-Null
try {
    Write-Host "Downloading $Asset..."
    Invoke-WebRequest -Uri $DownloadUrl -OutFile (Join-Path $TmpDir $Asset) -Headers @{ "User-Agent" = "bountui-installer" }
    Expand-Archive -Path (Join-Path $TmpDir $Asset) -DestinationPath $TmpDir -Force

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Move-Item -Path (Join-Path $TmpDir "bountui.exe") -Destination (Join-Path $InstallDir "bountui.exe") -Force
}
finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}

Write-Host "bountui $Version installed to $InstallDir\bountui.exe"

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($UserPath -split ';') -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path = "$env:Path;$InstallDir"
    Write-Host "note: $InstallDir was added to your user PATH (restart your terminal to pick it up)"
}

Write-Host "bountui will offer future releases in an update dialog on startup."
