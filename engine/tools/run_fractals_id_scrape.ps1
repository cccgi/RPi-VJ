$ErrorActionPreference = 'Stop'
$root = 'E:\__Projects\RPi-VJ'
$python = 'C:\Users\Admin\AppData\Local\Python\pythoncore-3.14-64\python.exe'
$script = Join-Path $root 'engine\tools\fetch_shadertoy_search.py'
$ids = Join-Path $root 'target\shadertoy-fractals\search_ids.json'
$log = Join-Path $root 'target\fractals-id-retry.log'

if (!(Test-Path -LiteralPath $ids)) {
    throw "Missing persisted ID list: $ids. Run the one-time discovery crawl first."
}

$count = @((Get-Content -LiteralPath $ids -Raw | ConvertFrom-Json)).Count
Write-Host "Retrying $count persisted fractal IDs; search-page discovery is disabled."
Write-Host "Live log: $log"
# The Python crawler creates its own temporary Edge profile and imports the
# authenticated Shadertoy cookies. Never point it at Edge's live User Data
# directory: a running Edge instance locks that profile.
Remove-Item Env:SHADERTOY_EDGE_PROFILE -ErrorAction SilentlyContinue

& $python $script 2>&1 | Tee-Object -FilePath $log
if ($LASTEXITCODE -ne 0) { throw "Fractal ID scrape exited with code $LASTEXITCODE" }
