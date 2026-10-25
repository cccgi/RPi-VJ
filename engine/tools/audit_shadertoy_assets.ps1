$ErrorActionPreference = 'Stop'
$root = 'E:\__Projects\RPi-VJ'
$python = 'C:\Users\Admin\AppData\Local\Python\pythoncore-3.14-64\python.exe'
$script = Join-Path $root 'engine\tools\shader_bank\fetch_assets.py'
$log = Join-Path $root 'target\shadertoy-asset-audit.log'

Write-Host 'Auditing every known Shadertoy asset without opening a browser or downloading files.'
& $python $script --method none --media 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { throw "Asset audit found missing files: see $log" }
