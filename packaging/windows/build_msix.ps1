<#
.SYNOPSIS
    Build the unsigned MSIX package of Harfiy for Microsoft Store submission.

.DESCRIPTION
    Stages harfiy.exe, the visual assets and the filled-in AppxManifest.xml, indexes the
    scale/targetsize asset variants into resources.pri (makepri) and packs everything
    with makeappx. The Store signs the package itself, so no certificate is needed.

.EXAMPLE
    ./packaging/windows/build_msix.ps1 -Version 1.2.3 -BinaryDir target/release -OutputDir dist
#>
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$BinaryDir,
    [Parameter(Mandatory = $true)][string]$OutputDir,
    # Must match "Product identity" in Partner Center exactly
    [string]$IdentityName = "AbdumannonShamsiyev.Harfiy",
    [string]$Publisher = "CN=B59EC37F-75F0-4379-86FB-39935700B327",
    [string]$PublisherDisplayName = "Abdumannon Shamsiyev"
)

$ErrorActionPreference = "Stop"

function Find-SdkTool([string]$Name) {
    $kits = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
    $tool = Get-ChildItem -Path $kits -Filter $Name -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } |
        Sort-Object { [version]($_.FullName -replace '^.*\\bin\\([\d.]+)\\.*$', '$1') } |
        Select-Object -Last 1
    if (-not $tool) { throw "$Name not found under $kits (install the Windows 10/11 SDK)" }
    return $tool.FullName
}

# MSIX versions have four parts, and the Store requires the last one to be 0
$numeric = ($Version -replace '^v', '') -replace '[-+].*$', ''
$parts = @($numeric.Split('.') | ForEach-Object { [int]$_ })
while ($parts.Count -lt 3) { $parts += 0 }
$msixVersion = "{0}.{1}.{2}.0" -f $parts[0], $parts[1], $parts[2]

$makeappx = Find-SdkTool "makeappx.exe"
$makepri = Find-SdkTool "makepri.exe"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = Join-Path (Resolve-Path $BinaryDir) "harfiy.exe"
if (-not (Test-Path $exe)) { throw "harfiy.exe not found in $BinaryDir" }

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$outDir = (Resolve-Path $OutputDir).Path
$work = Join-Path ([IO.Path]::GetTempPath()) ("harfiy-msix-" + [guid]::NewGuid())
$layout = Join-Path $work "layout"
New-Item -ItemType Directory -Force -Path $layout | Out-Null

try {
    Copy-Item $exe -Destination $layout
    Copy-Item (Join-Path $root "msix\Assets") -Destination $layout -Recurse

    $escape = { param($s) [Security.SecurityElement]::Escape($s) }
    $manifest = (Get-Content (Join-Path $root "msix\AppxManifest.xml") -Raw -Encoding UTF8).
        Replace('$IdentityName$', (& $escape $IdentityName)).
        Replace('$Publisher$', (& $escape $Publisher)).
        Replace('$PublisherDisplayName$', (& $escape $PublisherDisplayName)).
        Replace('$Version$', $msixVersion)
    [IO.File]::WriteAllText((Join-Path $layout "AppxManifest.xml"), $manifest, (New-Object Text.UTF8Encoding $false))

    $priConfig = Join-Path $work "priconfig.xml"
    & $makepri createconfig /cf $priConfig /dq uz-Latn /pv 10.0.0 /o | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "makepri createconfig failed ($LASTEXITCODE)" }
    # Keep every asset variant in the single resources.pri: split resource packs only
    # work inside an .msixbundle
    [xml]$config = Get-Content $priConfig -Raw
    $packaging = $config.SelectSingleNode("//packaging")
    if ($packaging) { [void]$packaging.ParentNode.RemoveChild($packaging) }
    $config.Save($priConfig)
    & $makepri new /pr $layout /cf $priConfig /mn (Join-Path $layout "AppxManifest.xml") /of (Join-Path $layout "resources.pri") /o | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "makepri new failed ($LASTEXITCODE)" }

    $package = Join-Path $outDir ("harfiy-v{0}-windows-x86_64-store.msix" -f $numeric)
    & $makeappx pack /d $layout /p $package /o | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed ($LASTEXITCODE)" }

    Write-Host "MSIX package: $package (version $msixVersion, identity $IdentityName)"
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}
