<div align="center">

<img src="assets/banner.svg" alt="AntiPod Banner" width="100%" />

**Native Rust + Tauri v2 launcher and profile manager for running multiple isolated Google Antigravity instances on Windows**

🌐 **Language:** **English** | [Русский](README.ru.md)

[![Release](https://img.shields.io/github/v/release/pirotehnikng-cloud/antipod?color=2e8540&label=Release)](https://github.com/pirotehnikng-cloud/antipod/releases)
[![Windows Build](https://img.shields.io/github/actions/workflow/status/pirotehnikng-cloud/antipod/release.yml?label=Windows%20Build)](https://github.com/pirotehnikng-cloud/antipod/actions)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%2F11%20(x64)-0078D6?logo=windows)](https://github.com/pirotehnikng-cloud/antipod/releases)
[![License](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)

[Download Windows Release](https://github.com/pirotehnikng-cloud/antipod/releases/latest) · [Release Features](#-release-features-v131) · [Quick Start](#-quick-start) · [CLI Tool](#-command-line-interface-antipod-cli)

</div>

---

**AntiPod** lets you run multiple isolated **Google Antigravity** windows (**IDE** or **Standalone App** mode) simultaneously under separate Google accounts without session, cookie, cache, or network conflicts. Built in pure Rust with minimal RAM overhead and full portable mode support.

<div align="center">

### Initial Tests: Running Two Isolated Antigravity Instances Side-by-Side

![AntiPod initial tests — two isolated Google Antigravity windows](assets/first-test.png)

*Two concurrent Google Antigravity instances running with isolated `user-data-dir`, environment variables, and independent sessions managed by AntiPod.*

</div>

---

## ✨ Release Features (`v1.3.1`)

Click any section below to expand full details and configuration options:

<details open>
<summary><b>1. Full Profile Isolation & Launch Modes (IDE and Standalone App)</b></summary>
<br>

- **Dedicated Data Directories:** Each profile gets an isolated folder under `AntiPodProfiles/<profile_name>/` (`user-data`, `home`, and optional `extensions` / `chrome-data`).
- **Dual Launch Modes:**
  - `ide` — full **Antigravity IDE** code editor with isolated user settings, themes, and project workspace.
  - `app` — standalone **Antigravity App** window focused on agent conversations.
- **Visual Window Identification:** Per-profile titlebar color customization (`titlebar_color`) injected into `settings.json` to easily distinguish active accounts.
- **Automatic Git Worktrees:** When opening the same Git repository across multiple profiles simultaneously, AntiPod automatically creates a separate `git worktree` for secondary profiles to prevent `.git/index.lock` conflicts.
- **Process Tree Management & RAM Monitoring:** Real-time memory and process count tracking per profile; clicking **Stop** cleanly terminates the entire Windows Job Object process tree.

</details>

<details>
<summary><b>2. Built-in Local Proxy Bridge (HTTP / HTTPS / SOCKS5 with Authentication)</b></summary>
<br>

- **Authenticated Proxy Support for Chromium:** Chromium-based apps do not accept `username:password` credentials inside `--proxy-server`. AntiPod spins up a dedicated local TCP proxy bridge on `127.0.0.1` per profile, automatically handling `Proxy-Authorization` headers and SOCKS5 handshakes.
- **DNS Leak Prevention:** Passes `--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1` so DNS resolution is routed through the tunnel rather than leaking locally.
- **Supported Proxy Formats:**
  - `http://user:pass@ip:port`
  - `https://user:pass@ip:port`
  - `socks5://user:pass@ip:port` (or `socks5h://...`)
  - `ip:port:user:pass` (shorthand format, defaults to HTTP)
  - `http://ip:port` or `socks5://ip:port` (unauthenticated)
- **"Test Proxy" Button:** Verifies proxy connectivity directly from the profile card or edit form before launching, detects authentication failures (`HTTP 407`), and displays **external IP, country, city, and latency (ms)**.

</details>

<details>
<summary><b>3. MostLogin Local API Integration & Custom Browser Selection</b></summary>
<br>

- **Native MostLogin Control (`http://127.0.0.1:30898`):**
  - Store your Local API endpoint and `Authorization` token directly inside AntiPod.
  - **Load Profiles** fetches available profiles via `/api/profile/getProfiles` and populates a dropdown selector in the profile editor.
  - **MostLogin** and **Close ML** buttons on each profile card open (`/api/browser/openBrowser`) and close (`/api/browser/closeProfiles`) the bound browser profile by `profileId`.
- **Custom Browser Executable (`.exe`):** Point any profile to an antidetect browser or portable browser executable. Non-Chromium launchers are started cleanly without conflicting `--user-data-dir` flags.
- **Automatic Browser Discovery:** If no custom path or MostLogin ID is configured, AntiPod automatically detects installed browsers in order: `Google Chrome → Microsoft Edge → Brave → Firefox`.

</details>

<details>
<summary><b>4. Automatic OAuth Link Interception to Clipboard & MostLogin</b></summary>
<br>

- **No System Default Browser Changes:** When **Intercept OAuth** is enabled in the top bar, AntiPod configures `workbench.externalBrowser` inside the profile's `IDE` `settings.json` to point to `AntiPod.exe` and passes the profile name via `ANTIPOD_PROFILE`.
- **Instant Background Handling:** Clicking Sign In inside Antigravity IDE routes the OAuth URL to a headless `AntiPod.exe` invocation (without spawning a second GUI window):
  1. Copies the OAuth URL straight to the Windows clipboard.
  2. If a **MostLogin** profile is bound, opens the URL directly inside the active MostLogin window via its CDP port (`/json/new`) or launches the profile via Local API.
  3. Reports the interception status in the AntiPod notification bar.

</details>

<details>
<summary><b>5. Portable Mode, System Tray & Autostart</b></summary>
<br>

- **True Portable Mode:** Place an empty `portable.flag` file (or an existing `profiles.toml`) next to `AntiPod.exe` to store all profiles, settings, and `state.json` locally in the application folder without touching `%APPDATA%`.
- **System Tray Integration:** Closing the main window minimizes AntiPod to the Windows system tray, allowing one-click profile launch/stop with live RAM usage, stopping all instances at once, and toggling Windows startup (`--minimized`).

</details>

---

## 🚀 Quick Start

1. Download the latest **`AntiPod-v1.3.1-windows-x64-portable.zip`** from [Releases](https://github.com/pirotehnikng-cloud/antipod/releases/latest) and extract it to a folder (e.g. `C:\AntiPod`).
2. *(Optional)* Keep `portable.flag` next to `AntiPod.exe` to run in portable mode.
3. Launch `AntiPod.exe`.
4. *(For MostLogin users)* In the **MostLogin Local API** panel, enter `http://127.0.0.1:30898`, paste your API token (e.g. `Bearer <your_token>`), and click **Load Profiles**.
5. In **Add or Update Profile**, enter a profile name, select `IDE` or `Standalone App`, bind a MostLogin profile (or pick a custom `.exe` browser), enter and verify your proxy with **Test Proxy**, and click **Save**.
6. Click **MostLogin** (or **Browser**) to open the browser profile, then click **Launch** to start an isolated Google Antigravity instance.

---

## 💻 Command-Line Interface (`antipod-cli`)

AntiPod includes `antipod.exe` for managing profiles from PowerShell or automated scripts:

<details>
<summary><b>Show CLI Commands</b></summary>
<br>

```powershell
# Generate a starter profiles.toml configuration file
antipod init

# List all configured profiles, bound browsers, and proxies
antipod list

# Launch an isolated Antigravity profile (with automatic local proxy bridge)
antipod launch work

# Open the bound MostLogin profile or browser for OAuth
antipod browser work

# Fetch and list profiles from MostLogin Local API
antipod mostlogin-list

# Test proxy connectivity by profile name or direct proxy URL
antipod test-proxy work
antipod test-proxy "socks5://user:pass@1.2.3.4:1080"

# Terminate the profile process tree
antipod stop work
