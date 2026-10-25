param(
    [string]$Profile,
    [string[]]$Url,
    [string]$File,
    [string]$Resume,
    [string]$Batch,
    [string]$HostName = 'rpi-vj3.local',
    [string]$User = 'rpi',
    [string]$IdentityFile,
    [int]$Seconds = 8,
    [switch]$RefreshIds,
    [switch]$NoDeploy
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..')).Path
$python = (Get-Command python -ErrorAction Stop).Source
$arguments = @((Join-Path $PSScriptRoot 'run_shader_pipeline.py'), '--host', $HostName,
               '--user', $User, '--seconds', $Seconds)
if ($Profile) { $arguments += @('--profile', $Profile) }
if ($File) { $arguments += @('--file', $File) }
if ($Resume) { $arguments += @('--resume', $Resume) }
foreach ($item in $Url) { $arguments += @('--url', $item) }
if ($Batch) { $arguments += @('--batch', $Batch) }
if ($IdentityFile) { $arguments += @('--identity', $IdentityFile) }
if ($RefreshIds) { $arguments += '--refresh-ids' }
if ($NoDeploy) { $arguments += '--no-deploy' }
& $python @arguments
if ($LASTEXITCODE -ne 0) { throw "Shader pipeline stopped; batch state is under $root\target\shader-pipeline" }
