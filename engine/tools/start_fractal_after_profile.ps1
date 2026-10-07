$ErrorActionPreference = 'SilentlyContinue'
$root = 'E:\__Projects\RPi-VJ'
$py = 'C:\Users\Admin\AppData\Local\Python\pythoncore-3.14-64\python.exe'
$script = Join-Path $root 'engine\tools\fetch_shadertoy_search.py'
$log = Join-Path $root 'target\fractals-crawl.log'
$stable = 0
$lastCount = -1
while ($true) {
    $profile = Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -match 'fetch_shadertoy_profile\.py' }
    $count = @(Get-ChildItem (Join-Path $root 'target\shadertoy-downloads') -Filter '*.glsl').Count
    if ($null -eq $profile -and $count -eq $lastCount) { $stable++ } else { $stable = 0 }
    $lastCount = $count
    if ($null -eq $profile -and $stable -ge 3) { break }
    Start-Sleep -Seconds 30
}
& $py $script *>> $log
