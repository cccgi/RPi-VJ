param(
    [Parameter(Mandatory=$true)][string]$InputFile,
    [string]$Name,
    [int]$CheckpointEvery = 150,
    [int]$ChallengeWait = 25,
    [int]$PrimeAttempts = 6
)
$ErrorActionPreference = 'Stop'
$root = 'E:\__Projects\RPi-VJ'
$python = 'C:\Users\Admin\AppData\Local\Python\pythoncore-3.14-64\python.exe'
if (-not (Test-Path -LiteralPath $InputFile)) { throw "Input file not found: $InputFile" }

$args = @((Join-Path $root 'engine\tools\fetch_shadertoy_user.py'), '--ids-file', (Resolve-Path -LiteralPath $InputFile),
    '--checkpoint-every', $CheckpointEvery, '--challenge-wait', $ChallengeWait, '--prime-attempts', $PrimeAttempts)
if ($Name) { $args += @('--name', $Name) }
& $python @args 2>&1 | Tee-Object -FilePath (Join-Path $root 'target\shadertoy-list-import.log')
if ($LASTEXITCODE -ne 0) { throw 'List import failed; inspect target\shadertoy-list-import.log' }
