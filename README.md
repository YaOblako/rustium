# Rustium

Rustium is a modern, privacy-focused desktop web browser built from the ground up in Rust. Designed with a strict Material Design 3 tonal surface architecture (Zero Borders / Strokes policy), Rustium integrates native request filtering powered by Brave's adblock engine, an intelligent Dark Reader engine with automated dark-surface detection, an accelerated multi-threaded download manager, WebExtensions support, and a local credential manager.

---

## Key Features

### 1. Material Design 3 Tonal Surface Architecture (Zero Borders)
- **Zero Border Policy:** Elements, tabs, cards, omnibox fields, dialogs, and sidebars are distinguished solely through Material Design 3 surface tonal elevations and state layers (`surfaceContainerLowest` through `surfaceContainerHighest`). No artificial borders, divider lines, or strokes are used.
- **Custom Integrated Titlebar:** Frameless window design where tabs sit directly at the top boundary of the window alongside native-feeling Windows 11 window controls (Minimize, Maximize, Close) and interactive drag zones.
- **Multi-Theme Engine:**
  - Material Design 3 Dark (Default)
  - Material Design 3 Light
  - AMOLED Black (Pure black for OLED displays)
  - Catppuccin Mocha (Developer-centric pastel palette)
  - Nord Polar Night (Arctic slate theme)
- **Dynamic Accent Generation:** Real-time generation of Primary, On-Primary, and Primary Container tonal palettes from customizable seed colors.
- **Tactile State Layers:** Custom Material Design 3 click ripple animations and hover/active feedback across all interactive controls.

### 2. Built-in Ad & Tracker Protection (Brave Engine + uBlock Origin)
- High-performance filtering engine utilizing `adblock-rust` (the filtering engine powering the Brave Browser).
- Support for EasyList filter rules, network blocking of tracking scripts, analytics, and intrusive ad networks.
- Cosmetic filter injection via dynamic CSS rules and live `MutationObserver` ad removal.
- **Scam & Phishing Interstitial Warning:** Integrated uBlock Origin badware and phishing domain scanner. Intercepts malicious URLs before navigation and presents a full-screen interstitial warning allowing safe return or explicit override.
- Omnibox shield badge with live blocked-request counter and per-domain whitelisting.

### 3. Intelligent Dark Reader Engine
- Dynamic client-side contrast and luminance inversion engine.
- **Automated Native Dark Detection:** Inspects document color schemes and computed background luminance (`0.2126R + 0.7152G + 0.0722B < 55`) to identify pre-existing dark websites (such as YouTube in dark mode, Discord, and GitHub). Pre-darkened sites are preserved to prevent double-inversion.
- Preserves natural colors for images (`<img>`), video players (`<video>`), `<canvas>`, and vector icons (`<svg>`).
- Real-time adjustment popover with per-site toggles, brightness control (50%–150%), and contrast control (50%–150%).

### 4. Accelerated Multi-Threaded Download Manager
- Concurrent file downloader powered by `tokio` and `reqwest`.
- Automatically queries servers via `HEAD` and probe requests for `Accept-Ranges: bytes` support and `Content-Length`.
- Pre-allocates files and splits large transfers into 2 to 16 concurrent byte ranges using non-blocking asynchronous tasks and localized file seeks (`SeekFrom::Start`).
- Graceful automatic fallback to single-threaded streaming when byte ranges are not supported by the host.
- Comprehensive control suite: real-time speed monitoring, pause, resume, cancel with disk cleanup, open file with default application, and show in folder.
- Background task protection: Warns the user with a confirmation dialog before exiting if active transfers are running.

### 5. WebExtensions Compatibility
- Unpacked extension loader supporting standard `manifest.json` configurations (Manifest V2 and V3).
- Injects custom user scripts (`content_scripts`) and stylesheets into matched URL patterns.
- Toolbar popover for extensions displaying active status, permissions overview, individual enable/disable switches, and extension management.

### 6. Local Credential & Password Manager
- Embedded SQLite storage for saved logins and credentials.
- Universal CSV password importer compatible with exports from Google Chrome, Mozilla Firefox, and Microsoft Edge.
- Built-in credentials viewer in Settings with options to view, manage, and delete stored entries.

### 7. Developer Tools & Custom Context Menu
- Native Chromium / Edge DevTools integration accessible via `F12` or the custom context menu.
- Material Design 3 right-click context menu offering Back, Forward, Reload, Copy Link Address, and Inspect Element.

---

## Architecture & Technology Stack

| Component | Technology | Description |
|---|---|---|
| Core Language | **Rust 2021 Edition** | High-performance memory-safe systems programming |
| Windowing & Event Loop | **Tao 0.37** | Cross-platform window creation and event processing |
| Web Rendering Engine | **Wry 0.57 + WebView2** | Hardware-accelerated native web rendering layer |
| Async Runtime | **Tokio 1.54** | Asynchronous task scheduling and thread pooling |
| HTTP & Networking | **Reqwest 0.13 + Hyper** | Async HTTP client supporting streaming and custom headers |
| Adblocking Core | **adblock-rust 0.13** | Brave's high-speed rule matching engine |
| Local Database | **Rusqlite 0.42** | Embedded SQLite for history, bookmarks, tabs, and logins |
| UI Chrome Layer | **HTML5, CSS3, ES6** | Borderless Material Design 3 shell using custom protocol |
| Windows Integration | **windows-sys 0.52 + winres** | Native Win32 message boxes, window hierarchy, and resources |

```
Rustium/
├── Cargo.toml               # Package configuration, features, and dependencies
├── build.rs                 # Windows resource compilation (icon and metadata)
├── installer.iss            # Inno Setup 6 packaging script
├── assets/
│   ├── icon.ico             # Multi-resolution application icon (16x16 to 256x256)
│   └── icon_32.rgba         # Raw RGBA icon buffer for Tao window initialization
├── src/
│   ├── main.rs              # Application entry point, event loop, and WebView coordinator
│   ├── lib.rs               # Library root export for integration testing
│   ├── adblock/             # Brave adblock engine, EasyList rules, and uBlock warnings
│   ├── dark_mode/           # Intelligent Dark Reader engine and DOM contrast analyzer
│   ├── downloader/          # Multi-threaded concurrent chunk downloader
│   ├── engine/              # Tab state management, User-Agent, and IPC message schemas
│   ├── extensions/          # WebExtensions loader, manifest parser, and content injectors
│   ├── storage/             # SQLite database layer for settings, history, bookmarks, and logins
│   └── ui/                  # Browser chrome and internal protocol pages
│       ├── chrome.html      # Top bar, integrated tabs, omnibox, and popover panels
│       └── pages/           # Internal rustium:// pages (newtab, settings, downloads, etc.)
├── tests/
│   └── rustium_tests.rs     # Automated test suite covering core engine modules
└── dist/                    # Compiled release installer output
```

---

## Keyboard Shortcuts

Rustium implements standard browser and Firefox-compatible keybindings:

| Shortcut | Action |
|---|---|
| `Ctrl + T` | Open a new tab |
| `Ctrl + W` | Close the active tab |
| `Ctrl + Tab` | Switch to the next tab |
| `Ctrl + Shift + Tab` | Switch to the previous tab |
| `Ctrl + R` / `F5` | Reload the active page |
| `Ctrl + L` / `Alt + D` | Focus the omnibox address bar |
| `Ctrl + H` | Open Browsing History (`rustium://history`) |
| `Ctrl + J` | Open Downloads Manager (`rustium://downloads`) |
| `Ctrl + B` | Open Bookmarks Manager (`rustium://bookmarks`) |
| `Ctrl + D` | Bookmark the current page |
| `Ctrl + ,` | Open Settings (`rustium://settings`) |
| `F12` | Toggle Developer Tools (Inspect Element) |
| `Alt + Left` | Navigate back in history |
| `Alt + Right` | Navigate forward in history |
| `Ctrl + Click` / `Middle Click` | Open link in a new background tab |

---

## Internal Protocol Pages

Rustium provides dedicated internal pages rendered directly through local templates:

- `rustium://newtab` — Home dashboard with search engine switcher, quick links, and privacy summary
- `rustium://settings` — Appearance themes, accent color picker, search engine configuration, multi-thread download concurrency, and password management
- `rustium://downloads` — Real-time download manager with thread counts, transfer speeds, and progress tracking
- `rustium://history` — Chronological browsing history with visit counts and search
- `rustium://bookmarks` — Saved bookmarks with quick navigation and deletion
- `rustium://extensions` — Unpacked extension manager and permissions inspector
- `rustium://warning` — uBlock Origin scam, phishing, and malware interstitial safety screen

---

## Building from Source

### Prerequisites

1. **Rust Toolchain:** Install the current stable Rust compiler (1.80+) via [rustup.rs](https://rustup.rs/):
   ```powershell
   rustup default stable-x86_64-pc-windows-msvc
   ```
2. **Microsoft C++ Build Tools:** Required for MSVC linking.
3. **Microsoft Edge WebView2 Runtime:** Pre-installed on Windows 10 (version 1803+) and Windows 11.
4. **Inno Setup 6 (Optional, for building the installer):**
   ```powershell
   winget install JRSoftware.InnoSetup
   ```

### Compilation

Clone the repository and run the test suite:

```powershell
git clone https://github.com/rustium/rustium.git
cd Rustium

# Verify compilation and run all unit and integration tests
cargo test
```

Build the optimized release binary:

```powershell
cargo build --release
```

The compiled binary will be located at `target\release\rustium.exe`.

### Packaging the Windows Installer

To compile the standalone Windows setup installer using Inno Setup:

```powershell
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer.iss
```

The resulting setup executable is generated at `dist\Rustium-Setup-v1.1.1.exe`.

---

## License

Rustium is distributed under the terms of both the **MIT License** and the **Apache License (Version 2.0)**, at your option.

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
