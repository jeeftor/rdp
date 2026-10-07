[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Target,
    [int]$Port = 3389
)

$stamp = Get-Date -Format 'yyyyMMddTHHmmss'
$outputPath = Join-Path $PSScriptRoot "windows-rdp-client-probe-$stamp.log"

Start-Transcript -Path $outputPath -Force | Out-Null

Write-Output "UTC: $((Get-Date).ToUniversalTime().ToString('o'))"
Write-Output "Current identity: $(whoami)"
Write-Output "Target: $Target`:$Port"

Write-Output "`n===== Windows and active identity ====="
Get-CimInstance Win32_OperatingSystem |
    Select-Object Caption, Version, BuildNumber, OSArchitecture | Format-List
Get-CimInstance Win32_ComputerSystem |
    Select-Object Name, Domain, PartOfDomain | Format-List
whoami /all

Write-Output "`n===== Network profile ====="
Get-NetConnectionProfile | Format-List Name, InterfaceAlias, NetworkCategory, IPv4Connectivity, IPv6Connectivity

Write-Output "`n===== Route ====="
Find-NetRoute -RemoteIPAddress $Target -ErrorAction SilentlyContinue | Format-List *

Write-Output "`n===== TCP probe ====="
Test-NetConnection -ComputerName $Target -Port $Port -InformationLevel Detailed | Format-List *

Write-Output "`n===== Saved RDP credentials ====="
cmdkey /list | Select-String -Pattern 'TERMSRV' | ForEach-Object { $_.Line }

Write-Output "`n===== Recent RDP client events ====="
try {
    $since = (Get-Date).AddMinutes(-30)
    Get-WinEvent -LogName 'Microsoft-Windows-TerminalServices-RDPClient/Operational' -MaxEvents 200 |
        Where-Object { $_.TimeCreated -ge $since } |
        Select-Object TimeCreated, Id, LevelDisplayName, Message |
        Format-List
}
catch {
    Write-Warning "Could not query the RDP client event log: $_"
}

Stop-Transcript | Out-Null
Write-Output "Saved: $outputPath"
