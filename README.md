# ViewMD

[![Release](https://img.shields.io/github/v/release/FXGears/viewmd?style=flat-square)](https://github.com/FXGears/viewmd/releases)
[![License](https://img.shields.io/github/license/FXGears/viewmd?style=flat-square)](LICENSE)
[![Binary Size](https://img.shields.io/badge/binary-652_KB-blue?style=flat-square)]()
[![No Electron](https://img.shields.io/badge/electron-none-green?style=flat-square)]()

**Under 1 MB. Under 100ms. Opens a markdown file. Closes when you're done.**

Every other markdown tool wants to be your editor, your notebook, your second brain, or your publishing pipeline. ViewMD just opens the file.

No editor. No tabs. No sidebar. No plugins. No settings. No config files. No internet. No telemetry. No decisions.

![ViewMD screenshot](resources/screenshot.png)

## Install

**Winget:**
```
winget install vRED.ViewMD
```

**Scoop:**
```
scoop bucket add viewmd https://github.com/FXGears/viewmd
scoop install viewmd
```

**Manual:** Download `viewmd.exe` from [Releases](https://github.com/FXGears/viewmd/releases). No installer. It's one file.

Set as default: right-click any `.md` file → Open with → select `viewmd.exe` → "Always use this app."

## How It Compares

| | **ViewMD** | *Typora* | *Marktext* | *Tinta* | *Obsidian* |
|---|---|---|---|---|---|
| Opens a markdown file | **✓** | *✓* | *✓* | *✓* | *✓* |
| Binary size | **652 KB** | *90 MB* | *180 MB* | *1.8 MB* | *300+ MB* |
| Startup time | **<100ms** | *~2s* | *~3s* | *~100ms* | *~4s* |
| Electron inside | **No** | *No* | *Yes* | *No* | *Yes* |
| Wants to manage your files | **No** | *No* | *No* | *No* | *Yes* |
| Has a settings page | **No** | *Yes* | *Yes* | *Yes* | *Yes* |
| Has plugin system | **No** | *No* | *Yes* | *No* | *Yes* |
| Requires account | **No** | *No* | *No* | *No* | *Optional (but it'll ask)* |
| Phones home | **No** | *No* | *No* | *No* | *Yes* |
| **Features you'll never use** | **0** | **47** | **63** | **28** | **∞** |

## Non-Goals

These will never be added:

- Editing
- Tabs
- File watching
- Themes
- Mermaid / LaTeX / diagrams
- Export
- Plugin system
- Cross-platform

If you want those things, use [Tinta](https://tinta.cc), [Markpad](https://github.com/alecdotdev/Markpad), or [Typora](https://typora.io). They're good at being everything. ViewMD is good at being nothing except fast.

## Specs

| | |
|---|---|
| Binary | 652 KB |
| Memory | ~30 MB |
| Cold start | <100ms |
| Runtime deps | None (WebView2 ships with Windows 10/11) |
| Language | Rust |
| Renderer | WebView2 + GitHub-dark CSS |
| License | GPL-3.0 |

## Install from Binary

1. Download `viewmd.exe` from [Releases](https://github.com/FXGears/viewmd/releases)
2. Copy to `C:\Program Files\ViewMD\` (or anywhere you like)
3. Right-click any `.md` file → Open with → Choose another app → browse to `viewmd.exe`
4. Check "Always use this app"

Done. No installer, no runtime, no dependencies.

## Building

```
cargo build --release
```

Requires Rust (stable) and Visual Studio Build Tools. Output: `target/release/viewmd.exe`
