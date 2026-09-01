# DEPRECATED - do not run.
# This script wrote an ssh config file whose Windows permissions OpenSSH
# rejects ("Bad owner or permissions"), which breaks EVERY ssh command,
# not just the alias. sync.ps1 now targets the Pi's IP directly instead.
# If you already ran it and ssh is broken:
#     Remove-Item "$env:USERPROFILE\.ssh\config" -Force
Write-Host "DEPRECATED - see comments. If ssh is broken, run:" -ForegroundColor Red
Write-Host '  Remove-Item "$env:USERPROFILE\.ssh\config" -Force' -ForegroundColor Yellow
