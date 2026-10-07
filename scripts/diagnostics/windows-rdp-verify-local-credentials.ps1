[CmdletBinding()]
param(
    [string]$UserName = 'rdp-test',
    [string]$Domain = $env:COMPUTERNAME,
    [ValidateSet('Interactive', 'Network', 'Both')]
    [string]$Mode = 'Both'
)

$ErrorActionPreference = 'Stop'
$stamp = Get-Date -Format 'yyyyMMddTHHmmss'
$outputPath = Join-Path $PSScriptRoot "windows-rdp-verify-local-credentials-$stamp.log"

if (-not ('RdpCredentialVerifier' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class RdpCredentialVerifier
{
    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    public static extern bool LogonUser(
        string username,
        string domain,
        string password,
        int logonType,
        int logonProvider,
        out IntPtr token);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool CloseHandle(IntPtr handle);
}
'@
}

Start-Transcript -Path $outputPath -Force | Out-Null
$passwordPointer = [IntPtr]::Zero
$token = [IntPtr]::Zero

try {
    Write-Output "UTC: $((Get-Date).ToUniversalTime().ToString('o'))"
    Write-Output "Testing local credential validation for: $Domain\$UserName"
    Write-Output "Mode: $Mode"
    Write-Output 'The password is not printed or stored in this log.'

    $securePassword = Read-Host "Password for $Domain\$UserName" -AsSecureString
    $passwordPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($securePassword)
    $password = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($passwordPointer)

    $logonProviderDefault = 0
    $tests = @()
    if ($Mode -eq 'Interactive' -or $Mode -eq 'Both') {
        $tests += [pscustomobject]@{ Name = 'interactive local logon'; Type = 2 }
    }
    if ($Mode -eq 'Network' -or $Mode -eq 'Both') {
        $tests += [pscustomobject]@{ Name = 'network-style local authentication'; Type = 3 }
    }

    $failed = $false
    foreach ($test in $tests) {
        $token = [IntPtr]::Zero
        $success = [RdpCredentialVerifier]::LogonUser(
            $UserName,
            $Domain,
            $password,
            $test.Type,
            $logonProviderDefault,
            [ref]$token)

        if ($success) {
            Write-Output "SUCCESS: Windows accepted these credentials for $($test.Name)."
        }
        else {
            $failed = $true
            $errorCode = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
            $errorMessage = (New-Object ComponentModel.Win32Exception($errorCode)).Message
            Write-Error "FAILED: Windows rejected $($test.Name). Win32 error ${errorCode}: $errorMessage"
        }

        if ($token -ne [IntPtr]::Zero) {
            [void][RdpCredentialVerifier]::CloseHandle($token)
            $token = [IntPtr]::Zero
        }
    }
    $password = $null

    if ($failed) { exit 1 }
    exit 0
}
finally {
    if ($token -ne [IntPtr]::Zero) {
        [void][RdpCredentialVerifier]::CloseHandle($token)
    }
    if ($passwordPointer -ne [IntPtr]::Zero) {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($passwordPointer)
    }
    Stop-Transcript | Out-Null
    Write-Output "Saved: $outputPath"
}
