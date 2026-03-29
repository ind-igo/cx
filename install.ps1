#!/usr/bin/env pwsh
$ErrorActionPreference = 'Stop'

$Repo = "ind-igo/cx"
$Binary = "cx.exe"
$InstallDir = "$env:LOCALAPPDATA\cx\bin"

# Detect architecture
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($Arch) {
    'X64'   { $Target = "x86_64-pc-windows-msvc" }
    default { Write-Error "Unsupported architecture: $Arch"; exit 1 }
}

$Url = "https://github.com/$Repo/releases/latest/download/cx-$Target.zip"

Write-Host "Installing cx ($Target)..."

$TmpDir = New-TemporaryFile | ForEach-Object {
    Remove-Item $_
    New-Item -ItemType Directory -Path "$_.d"
}

try {
    $ZipPath = Join-Path $TmpDir "cx.zip"
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing
    Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    Copy-Item (Join-Path $TmpDir $Binary) (Join-Path $InstallDir $Binary) -Force

    # Add to PATH if not already there
    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $PathEntries = if ($UserPath) { $UserPath -split ';' } else { @() }
    if ($InstallDir -notin $PathEntries) {
        $NewPath = if ($UserPath) { "$UserPath;$InstallDir" } else { $InstallDir }
        [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
        Write-Host "Added $InstallDir to your PATH (restart your terminal to pick it up)"
    }

    Write-Host "cx installed to $InstallDir\$Binary"
} finally {
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
}
