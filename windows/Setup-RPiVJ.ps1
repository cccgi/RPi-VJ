param([switch]$NoLaunch)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$exe = Join-Path $root 'RPi-VJ.exe'
$catalogPath = Join-Path $root 'target\shader-bank\catalog.json'
$assetManifestPath = Join-Path $root 'target\shader-bank\assets\manifest.json'
if (-not [Environment]::Is64BitOperatingSystem) {
    throw 'This package requires 64-bit Windows 10 or 11.'
}
if (-not (Test-Path -LiteralPath $exe)) { throw "Application executable is missing: $exe" }
if (-not (Test-Path -LiteralPath $catalogPath)) { throw "Shader catalog is missing: $catalogPath" }
$catalog = Get-Content -Raw -LiteralPath $catalogPath | ConvertFrom-Json
$missingShaders = @()
foreach ($shader in $catalog.shaders) {
    if (-not $shader.path) { continue }
    $relative = $shader.path.Replace('/', '\')
    if (-not (Test-Path -LiteralPath (Join-Path $root $relative) -PathType Leaf)) {
        $missingShaders += $shader.id
    }
}
if ($missingShaders.Count -gt 0) {
    throw "$($missingShaders.Count) shader source files are missing. Re-extract the full ZIP before setup."
}
$missingAssets = @()
if (Test-Path -LiteralPath $assetManifestPath) {
    $manifest = Get-Content -Raw -LiteralPath $assetManifestPath | ConvertFrom-Json
    foreach ($asset in $manifest.assets) {
        if ($asset.status -ne 'ok' -or -not $asset.local_file) { continue }
        $relative = $asset.local_file.Replace('/', '\')
        if (-not (Test-Path -LiteralPath (Join-Path (Join-Path $root 'target\shader-bank\assets') $relative) -PathType Leaf)) {
            $missingAssets += $asset.source_path
        }
    }
}
if ($missingAssets.Count -gt 0) {
    throw "$($missingAssets.Count) texture/media asset files are missing. Re-extract the full ZIP before setup."
}
Write-Host "Verified $($catalog.shaders.Count) shader records and the catalog's local assets."
$runtime = @(
    (Join-Path $env:WINDIR 'System32\vcruntime140.dll'),
    (Join-Path $env:WINDIR 'System32\msvcp140.dll')
)
if (@($runtime | Where-Object { -not (Test-Path -LiteralPath $_) }).Count -gt 0) {
    Write-Host 'Installing the official Microsoft Visual C++ x64 runtime...'
    $installer = Join-Path $env:TEMP ('rpi-vj-vcredist-' + [guid]::NewGuid().ToString('N') + '.exe')
    try {
        Invoke-WebRequest -Uri 'https://aka.ms/vc14/vc_redist.x64.exe' -OutFile $installer
        $signature = Get-AuthenticodeSignature -LiteralPath $installer
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'Microsoft') {
            throw 'The downloaded VC++ installer did not pass Microsoft signature validation.'
        }
        $process = Start-Process -FilePath $installer -ArgumentList '/install /quiet /norestart' -Verb RunAs -Wait -PassThru
        if ($process.ExitCode -notin @(0, 3010)) {
            throw "Microsoft VC++ runtime installer returned exit code $($process.ExitCode)."
        }
        if ($process.ExitCode -eq 3010) {
            Write-Warning 'Windows requested a restart to finish installing the runtime.'
        }
    } finally {
        Remove-Item -LiteralPath $installer -Force -ErrorAction SilentlyContinue
    }
} else {
    Write-Host 'Microsoft Visual C++ runtime is already present.'
}
Write-Host 'Shader rendering runs locally in the Rust app; no browser is used.'
Write-Host 'For audio-reactive visuals, connect an audio input in the app. Use a current GPU driver for OpenGL.'
if (-not $NoLaunch) {
    $env:RPI_VJ_ROOT = $root
    Start-Process -FilePath $exe -WorkingDirectory $root
}
