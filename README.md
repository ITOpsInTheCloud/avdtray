# AVD Tray

A lightweight, open replacement for **frxtray.exe**, the FSLogix Profile Status tray tool that Microsoft deprecated on 22 Aug 2023 and removed from FSLogix 25.02 (retired 11 Feb 2025).

* Single native `avdtray.exe` (~400–700 KB), written in Rust on the raw Win32 API
* **No runtime, no VC++ redistributable, no third-party crates.** It builds offline.
* Runs as the signed-in user (`asInvoker`), needs no admin rights, and is per-session on multi-session hosts
* Windows 10/11, Windows Server 2016+ (x64 and ARM64)

## What it shows

**Tray icon (traffic light)**, refreshed every 10 s by reading a handful of registry values:

| Icon | Meaning |
|---|---|
| 🟢 green dot | Profile (and ODFC, if enabled) container attached |
| 🟡 amber "!" | Not attached for a *normal* reason (local profile exists, excluded group, …) or attach still in progress |
| 🔴 red "–" | FSLogix error status (1–28), or the `frxsvc` service isn't running |
| ⚪ grey ring | FSLogix not installed or not enabled on this host |

The tooltip shows the one-line state for each container. A notification balloon appears when the state turns red.

**Status window** (left-click the icon). It's a painted, DPI-aware window that follows the Windows light/dark setting:
* **Header cards:** computer, user, session ID and the overall status
* **Profile Container / Office Container (ODFC) tabs:**
  * Status, Reason and Error in plain English ([Microsoft code reference](https://learn.microsoft.com/fslogix/troubleshooting-error-codes))
  * Session key, VHD(X) path and type, read/write vs read-only, size and file system
  * A free/used space donut chart
  * Values that don't fit (long session keys, UNC paths) are shortened with "…". Hover over them to see the full text in a tooltip.
* **Session Details tab:** every value in the user's `Sessions\<SID>` keys, the state and version of each FSLogix service and driver, and the log paths
* **Report tab:** the full plain-text report, which is the screen-reader-friendly view
* **Keyboard:** Ctrl+Tab / Ctrl+Shift+Tab switch tabs, and Esc closes the window

**Right-click menu:** Show status, Refresh, Copy report to clipboard, open the latest Profile or ODFC log, open the logs folder, open the FSLogix event log (`Microsoft-FSLogix-Apps/Operational`), run `frx.exe list-redirects`, About, Exit.

Logs are only opened when you ask. Nothing is tailed in the background, which avoids the I/O load that frxtray was criticised for.

## Data sources

| What | Where |
|---|---|
| Profile config | `HKLM\SOFTWARE\FSLogix\Profiles` → `Enabled` |
| Profile session state | `HKLM\SOFTWARE\FSLogix\Profiles\Sessions\<SID>` → `Status`, `Reason`, `Error`, … |
| ODFC config | `HKLM\SOFTWARE\Policies\FSLogix\ODFC` → `Enabled` |
| ODFC session state | `HKLM\SOFTWARE\Policies\FSLogix\ODFC\Sessions\<SID>` (fallback `HKCU\Software\FSLogix\ODFC\Sessions`) |
| Install path | `HKLM\SOFTWARE\FSLogix\Apps` → `InstallPath` (fallback `%ProgramFiles%\FSLogix\Apps`) |
| Logs | `%ProgramData%\FSLogix\Logs\{Profile,ODFC}` |

## Build

**On Windows** (one time: install Rust from https://rustup.rs with the MSVC build tools):

```powershell
cargo build --release
# -> target\release\avdtray.exe
```

For ARM64: `rustup target add aarch64-pc-windows-msvc` and then `cargo build --release --target aarch64-pc-windows-msvc`.

**Without a local toolchain:** push this folder to a GitHub repo. The included workflow (`.github/workflows/build.yml`) builds x64 and ARM64 and uploads `avdtray.exe` as build artifacts.

## Command line

```
avdtray.exe [--interval <seconds>] [--show] [--no-notify]
```

* `--interval`: refresh period, 2–3600 s (default 10)
* `--show`: open the status window on start
* `--no-notify`: no balloon on errors

Starting it a second time in the same session opens the running instance's window.

## Deploy

Copy `avdtray.exe` to e.g. `C:\Program Files\AVDTray\`, then pick one:

* **Helpdesk / on demand:** users or admins just run it.
* **Autostart for all users:**
  ```powershell
  New-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' `
    -Name 'AVDTray' -Value '"C:\Program Files\AVDTray\avdtray.exe"' -PropertyType String -Force
  ```
  (or GPO / Intune with the same Run key, or a shortcut in the All Users Startup folder)
* **Specific users only:** use the `HKCU\...\Run` key via GPO item-level targeting.

## Notes

* Not affiliated with or supported by Microsoft.
* `frx.exe list-redirects` may need elevation on some hosts. Its output, including any error, is shown as-is.
* Status codes 0/100/200/300 are "normal"; Reason codes only apply to normal statuses (per Microsoft docs).
