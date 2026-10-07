[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$AccountName,
    [int]$Minutes = 30,
    [int]$MaxEvents = 200
)

$ErrorActionPreference = 'Continue'
$stamp = Get-Date -Format 'yyyyMMddTHHmmss'
$outputPath = Join-Path $PSScriptRoot "windows-rdp-debug-$stamp.log"
$since = (Get-Date).AddMinutes(-$Minutes)

Start-Transcript -Path $outputPath -Force | Out-Null

function Write-Section {
    param([string]$Title)
    Write-Output "`n===== $Title ====="
}

function Write-EventLog {
    param([string]$LogName)
    Write-Section $LogName
    try {
        Get-WinEvent -LogName $LogName -MaxEvents $MaxEvents |
            Where-Object { $_.TimeCreated -ge $since } |
            Select-Object TimeCreated, Id, LevelDisplayName, Message |
            Format-List
    }
    catch {
        Write-Warning $_
    }
}

function Write-UserRightsAssignment {
    $policyPath = Join-Path $env:TEMP "rdp-user-rights-$PID.inf"
    $rights = @(
        'SeRemoteInteractiveLogonRight',
        'SeDenyRemoteInteractiveLogonRight',
        'SeNetworkLogonRight',
        'SeDenyNetworkLogonRight',
        'SeInteractiveLogonRight',
        'SeDenyInteractiveLogonRight'
    )

    Write-Section 'Effective user-right assignments (local security policy export)'
    try {
        & secedit.exe /export /cfg $policyPath /areas USER_RIGHTS /quiet
        Write-Output "secedit exit code: $LASTEXITCODE"
        if (-not (Test-Path -LiteralPath $policyPath)) {
            Write-Warning "secedit did not create $policyPath"
            return
        }

        $matches = Get-Content -LiteralPath $policyPath | Where-Object {
            foreach ($right in $rights) {
                if ($_ -like "$right=*") { return $true }
            }
            return $false
        }
        if ($matches) {
            $matches
        }
        else {
            Write-Warning 'No requested user-right assignments were present in the exported policy.'
        }
    }
    catch {
        Write-Warning "Could not export user-right assignments: $_"
    }
    finally {
        if (Test-Path -LiteralPath $policyPath) {
            Remove-Item -LiteralPath $policyPath -Force
        }
    }
}

Write-Section 'Collection details'
Write-Output "UTC: $((Get-Date).ToUniversalTime().ToString('o'))"
Write-Output "Account queried: $AccountName"
Write-Output "Event window: last $Minutes minute(s)"
Write-Output "Maximum events per RDP log: $MaxEvents"

Write-Section 'RDP service and listener'
Get-Service -Name TermService | Format-List Name, Status, StartType
Get-NetTCPConnection -State Listen -LocalPort 3389 -ErrorAction SilentlyContinue |
    Format-Table -AutoSize LocalAddress, LocalPort, OwningProcess

Write-Section 'Windows and active identity'
Get-CimInstance Win32_OperatingSystem |
    Select-Object Caption, Version, BuildNumber, OSArchitecture, LastBootUpTime | Format-List
Get-CimInstance Win32_ComputerSystem |
    Select-Object Name, Domain, PartOfDomain, DomainRole | Format-List
whoami /all

Write-Section 'RDP registry settings'
Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server' |
    Select-Object fDenyTSConnections | Format-List
Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Terminal Server\WinStations\RDP-Tcp' |
    Select-Object UserAuthentication, SecurityLayer, PortNumber | Format-List

Write-Section 'Inbound firewall rules for TCP 3389'
try {
    Get-NetFirewallRule -Direction Inbound -Enabled True -ErrorAction Stop |
        Get-NetFirewallPortFilter |
        Where-Object { $_.Protocol -eq 'TCP' -and $_.LocalPort -eq '3389' } |
        Format-List *
}
catch {
    Write-Warning "Could not query firewall rules: $_"
}

Write-Section 'Local authentication and NTLM policy values'
Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Lsa' -ErrorAction SilentlyContinue |
    Select-Object LmCompatibilityLevel, NoLmHash, LimitBlankPasswordUse | Format-List
Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Lsa\MSV1_0' -ErrorAction SilentlyContinue |
    Select-Object RestrictReceivingNTLMTraffic, AuditReceivingNTLMTraffic | Format-List

Write-Section 'Account state'
Get-CimInstance Win32_UserAccount -Filter "Name='$AccountName' AND LocalAccount=True" |
    Select-Object Name, LocalAccount, Disabled, Lockout, PasswordExpires, Status, SID |
    Format-List
net accounts

Write-Section 'Local RDP and administrator group membership'
foreach ($groupName in @('Remote Desktop Users', 'Administrators')) {
    Write-Output "--- $groupName ---"
    try {
        Get-LocalGroupMember -Group $groupName -ErrorAction Stop |
            Select-Object Name, ObjectClass, PrincipalSource | Format-Table -AutoSize
    }
    catch {
        Write-Warning "Could not query ${groupName}: $_"
    }
}

Write-UserRightsAssignment

Write-Section 'Computer Group Policy summary'
try {
    gpresult.exe /r /scope computer
    Write-Output "gpresult exit code: $LASTEXITCODE"
}
catch {
    Write-Warning "Could not query computer Group Policy: $_"
}

Write-Section 'Recent security authentication events (4624, 4625, 4740, 4776)'
try {
    Get-WinEvent -FilterHashtable @{ LogName = 'Security'; Id = 4624, 4625, 4740, 4776; StartTime = $since } |
        Select-Object TimeCreated, Id, LevelDisplayName, Message |
        Format-List
}
catch {
    Write-Warning $_
}

Write-EventLog 'Microsoft-Windows-TerminalServices-RemoteConnectionManager/Operational'
Write-EventLog 'Microsoft-Windows-TerminalServices-LocalSessionManager/Operational'
Write-EventLog 'Microsoft-Windows-RemoteDesktopServices-RdpCoreTS/Operational'

Stop-Transcript | Out-Null
Write-Output "Saved: $outputPath"
