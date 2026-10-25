param([Parameter(Mandatory=$true)][string]$User, [switch]$RefreshIds, [int]$MaxPages = 200, [int]$CheckpointEvery = 150, [int]$ChallengeWait = 25, [int]$PrimeAttempts = 6)
$ErrorActionPreference = 'Stop'
$root = 'E:\__Projects\RPi-VJ'; $python = 'C:\Users\Admin\AppData\Local\Python\pythoncore-3.14-64\python.exe'
$args = @((Join-Path $root 'engine\tools\fetch_shadertoy_user.py'), $User, '--max-pages', $MaxPages, '--checkpoint-every', $CheckpointEvery, '--challenge-wait', $ChallengeWait, '--prime-attempts', $PrimeAttempts)
if ($RefreshIds) { $args += '--refresh-ids' }
& $python @args 2>&1 | Tee-Object -FilePath (Join-Path $root "target\shadertoy-user-$User.log")
if ($LASTEXITCODE -ne 0) { throw "User import failed; inspect target\shadertoy-user-$User.log" }
