$ErrorActionPreference = 'Stop'
$root = 'E:\__Projects\RPi-VJ'
$python = 'C:\Users\Admin\AppData\Local\Python\pythoncore-3.14-64\python.exe'
$script = Join-Path $root 'engine\tools\shader_bank\fetch_assets.py'
$log = Join-Path $root 'target\shadertoy-asset-recovery.log'

Write-Host 'Auditing then recovering missing Shadertoy texture, cubemap, audio, and video assets across the entire bank.'
Write-Host "Live log: $log"
& $python $script --method browser --interactive --import-cookies --media --delay 3 2>&1 |
    Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) {
    throw "Asset recovery exited with code $LASTEXITCODE. Review $log."
}
