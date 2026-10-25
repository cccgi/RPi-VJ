$ErrorActionPreference = 'Stop'
$root = 'E:\__Projects\RPi-VJ'
$python = 'C:\Users\Admin\AppData\Local\Python\pythoncore-3.14-64\python.exe'
$log = Join-Path $root 'target\shadertoy-user-diatribes.log'
Remove-Item Env:SHADERTOY_EDGE_PROFILE -ErrorAction SilentlyContinue
& $python (Join-Path $root 'engine\tools\fetch_shadertoy_user.py') 'diatribes' '--max-pages' '99' 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { throw "Diatribes scrape failed: see $log" }
