# FreeRDP diagnostics

Run these helpers with Bash on your Linux workstation. Logs default to
`./rdp-diagnostics-logs`; set `RDP_DIAGNOSTICS_LOG_DIR` to choose another
directory. Logs can contain hosts, usernames, and system information. Keep
them local and review them before sharing.

```bash
bundle="$HOME/freerdp-portable-x86_64-3.30.0"
bash scripts/diagnostics/00-run-readonly-checks.sh "$bundle" windows.example.invalid
bash scripts/diagnostics/03-monitors.sh "$bundle"
```

The read-only checks collect local facts and test reachability of the supplied
host; they do not open an RDP session. Monitor IDs are specific to your current
display session. You can set `SDL_VIDEODRIVER=wayland` or `SDL_VIDEODRIVER=x11`
to compare backends.

For connection diagnostics, explicitly supply your host and username:

```bash
RDP_HOST=windows.example.invalid RDP_USER=USERNAME bash scripts/diagnostics/configure.sh
bash scripts/diagnostics/run-all.sh "$bundle" windows.example.invalid "$HOME/.config/freerdp/rdp-test.args" 0,1
```

`configure.sh` prompts for a password and writes a mode-600 local argument
file. It is only for diagnostics: the profile uses trace logging and temporarily
ignores server certificate verification. After confirming server identity,
replace `/cert:ignore` with `/cert:tofu` or use the issuing CA and correct DNS
name. Set `RDP_SECURITY_MODE=tls` only for an intentional TLS comparison; the
default is NLA. Do not store credentials in the checkout or share argument
files. Normal `rdpctl` usage does not store passwords.

`run-all.sh` opens minimal and multi-monitor RDP sessions and continues after
failures to collect evidence. Use `00-run-readonly-checks.sh` when you only
want local checks.

Windows helpers take explicit targets or account names:

```powershell
.\windows-rdp-client-probe.ps1 -Target windows.example.invalid
.\windows-rdp-debug.ps1 -AccountName USERNAME -Minutes 30
.\windows-rdp-verify-local-credentials.ps1 -UserName USERNAME
```

The server report may need an Administrator PowerShell session. Its reports
include machine, account, and policy details; keep them local. The separate
`windows-rdp-create-test-user.ps1` helper creates a test account and should only
be run when you intend that change. It does not alter an existing account
unless you explicitly supply `-ResetPassword`.
