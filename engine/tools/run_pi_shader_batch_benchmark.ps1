param(
    [Parameter(Mandatory=$true)][string]$BatchName,
    [int]$Seconds = 8,
    [string]$HostName = 'rpi-vj3.local',
    [string]$User = 'rpi',
    [string]$IdentityFile
)
$ErrorActionPreference = 'Stop'
$root = 'E:\__Projects\RPi-VJ'
if ($BatchName -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$') { throw 'BatchName may contain only letters, digits, dot, underscore, and hyphen' }
$batch = Join-Path $root "target\shader-pipeline\$BatchName"
$native = Join-Path $batch 'native'
$ids = Join-Path $batch 'ids.json'
if (-not (Test-Path -LiteralPath $native) -or -not (Test-Path -LiteralPath $ids)) { throw "Batch is not prepared: $BatchName" }

$destination = "$User@$HostName"
$stage = "/srv/rpi-vj-batch-bench/$BatchName"
$remoteReport = "$stage/report.json"
$localReport = Join-Path $batch 'pi-benchmark.json'
$archive = Join-Path $env:TEMP "rpi-vj-$BatchName-$PID.tar"
$sshArgs = @()
if ($IdentityFile) {
    if (-not (Test-Path -LiteralPath $IdentityFile)) { throw "Identity file not found: $IdentityFile" }
    $sshArgs = @('-i', (Resolve-Path -LiteralPath $IdentityFile).Path)
}

# Candidate shaders stay outside /srv/rpi-vj-native, so a benchmark cannot
# replace the live review bank. The Pi script refuses to run while the live
# native services own KMS/DRM and the finally block restores them.
try {
    # One SCP transfer replaces three independent password prompts. Archive
    # members retain their paths, so no candidate touches the live Pi bank.
    & tar -cf $archive -C $batch 'native' 'ids.json' -C $root 'engine/tools/benchmark_native_shaders.py'
    if ($LASTEXITCODE -ne 0) { throw 'Could not create benchmark staging archive' }
    Write-Host 'Uploading benchmark batch (one SSH authentication)...'
    & scp @sshArgs $archive "$destination`:/tmp/rpi-vj-batch-$BatchName.tar"
    if ($LASTEXITCODE -ne 0) { throw 'Staging the Pi benchmark failed' }

    # One root shell owns the entire transaction.  This avoids a second sudo
    # prompt after a long benchmark exceeds sudo's timestamp timeout; it also
    # ensures that service restoration happens before that root shell exits.
    $rootScript = "mkdir -p `"$stage`"; tar -xf `"/tmp/rpi-vj-batch-$BatchName.tar`" -C `"$stage`"; rm -f `"/tmp/rpi-vj-batch-$BatchName.tar`"; systemctl stop rpi-vj-native rpi-vj-controller; python3 `"$stage/engine/tools/benchmark_native_shaders.py`" --i-understand-idle --seconds $Seconds --shader-dir `"$stage/native`" --shader-list `"$stage/ids.json`" --out `"$remoteReport`"; code=`$?; systemctl start rpi-vj-native rpi-vj-controller; exit `$code"
    $remote = "sudo sh -c '$rootScript'"
    Write-Host 'Running idle Pi benchmark (one SSH authentication)...'
    & ssh @sshArgs -tt $destination $remote
    if ($LASTEXITCODE -ne 0) { throw 'Pi benchmark failed; inspect its console output before publishing' }
    Write-Host 'Downloading benchmark report (one SSH authentication)...'
    & scp @sshArgs "$destination`:$remoteReport" $localReport
    if ($LASTEXITCODE -ne 0) { throw 'Pi benchmark report copy failed' }
}
finally {
    Remove-Item -LiteralPath $archive -Force -ErrorAction SilentlyContinue
}
Write-Host "BENCHMARK_REPORT $localReport"
