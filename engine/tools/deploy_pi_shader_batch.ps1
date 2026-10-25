param(
    [Parameter(Mandatory=$true)][string]$ReviewDirectory,
    [string]$HostName = 'rpi-vj3.local',
    [string]$User = 'rpi',
    [switch]$KeepServicesStopped,
    [switch]$ReplaceLive
)
$ErrorActionPreference = 'Stop'
$resolved = (Resolve-Path -LiteralPath $ReviewDirectory).Path
if (-not (Test-Path -LiteralPath (Join-Path $resolved 'show.json'))) { throw "show.json missing from $resolved" }
if (-not $ReplaceLive) {
    throw 'Refusing to replace the live show. This review package must be merged as additional pages first. Use -ReplaceLive only for an explicit full-bank replacement.'
}

# Credentials remain with the user's SSH agent/client. This is deliberately an
# explicit full-bank replacement; normal batch publishing must merge pages into
# a preserved show instead of overwriting it.
$destination = "$User@$HostName"
& ssh $destination 'sudo systemctl stop rpi-vj-controller rpi-vj-native'
try {
    & ssh $destination 'mkdir -p /tmp/rpi-vj-review'
    & scp -r (Join-Path $resolved '*') "$destination`:/tmp/rpi-vj-review/"
    if ($LASTEXITCODE -ne 0) { throw 'SCP failed' }
    & ssh $destination 'sudo mkdir -p /srv/rpi-vj-native; sudo cp -a /tmp/rpi-vj-review/. /srv/rpi-vj-native/; sudo rm -rf /tmp/rpi-vj-review'
    if ($LASTEXITCODE -ne 0) { throw 'Pi install failed' }
}
finally {
    if (-not $KeepServicesStopped) { & ssh $destination 'sudo systemctl start rpi-vj-native rpi-vj-controller' }
}
Write-Host "Pi review bank installed from $resolved"
