[CmdletBinding()]
param(
    [string]$UserName = 'rdp-test',
    [switch]$ResetPassword
)

$ErrorActionPreference = 'Stop'
$stamp = Get-Date -Format 'yyyyMMddTHHmmss'
$outputPath = Join-Path $PSScriptRoot "windows-rdp-create-test-user-$stamp.log"
Start-Transcript -Path $outputPath -Force | Out-Null

try {
    Write-Output "UTC: $((Get-Date).ToUniversalTime().ToString('o'))"
    Write-Output "Requested local user: $UserName"
    Write-Output "Reset existing password: $ResetPassword"
    $existing = Get-LocalUser -Name $UserName -ErrorAction SilentlyContinue

    if ($existing -and -not $ResetPassword) {
        Write-Output "Local user '$UserName' already exists; it was not changed."
        Write-Output "Use -ResetPassword to choose a new password for this diagnostic account."
    }
    else {
        $password = Read-Host "Password for $UserName" -AsSecureString
        if ($existing) {
            Set-LocalUser -Name $UserName -Password $password
            Write-Output "Password replaced for existing local user '$UserName'."
        }
        else {
            New-LocalUser -Name $UserName -Password $password -FullName 'RDP diagnostic account' |
                Out-Null
            Write-Output "Created local user '$UserName'."
        }
    }

    $member = Get-LocalGroupMember -Group 'Remote Desktop Users' -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match "\\$([regex]::Escape($UserName))$" }
    if (-not $member) {
        Add-LocalGroupMember -Group 'Remote Desktop Users' -Member $UserName
        Write-Output "Added '$UserName' to Remote Desktop Users."
    }
    else {
        Write-Output "'$UserName' is already in Remote Desktop Users."
    }

    Get-LocalUser -Name $UserName |
        Select-Object Name, Enabled, PasswordExpires, LastLogon, SID | Format-List
}
catch {
    Write-Error $_
    exit 1
}
finally {
    Stop-Transcript | Out-Null
    Write-Output "Saved: $outputPath"
}
