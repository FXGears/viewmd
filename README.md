# ViewMD

[![Release](https://img.shields.io/github/v/release/FXGears/viewmd?style=flat-square)](https://github.com/FXGears/viewmd/releases)
[![License](https://img.shields.io/github/license/FXGears/viewmd?style=flat-square)](LICENSE)
[![Binary Size](https://img.shields.io/badge/binary-652_KB-blue?style=flat-square)]()
[![No Electron](https://img.shields.io/badge/electron-none-green?style=flat-square)]()

A markdown viewer for Windows. Nothing else.

![ViewMD screenshot](resources/screenshot.png)

## How It Compares

| | ViewMD | Typora | Marktext | Tinta | Obsidian |
|---|---|---|---|---|---|
| Opens a markdown file | ✓ | ✓ | ✓ | ✓ | ✓ |
| Binary size | **652 KB** | 90 MB | 180 MB | 1.8 MB | 300+ MB |
| Startup time | <100ms | ~2s | ~3s | ~100ms | ~4s |
| Electron inside | No | No | Yes | No | Yes |
| Wants to manage your files | No | No | No | No | Yes |
| Has a settings page | No | Yes | Yes | Yes | Yes |
| Has plugin system | No | No | Yes | No | Yes |
| Requires account | No | No | No | No | Optional (but it'll ask) |
| Phones home | No | Yes | No | No | Yes |
| Features you'll never use | 0 | 47 | 63 | 28 | ∞ |

## Philosophy

ViewMD does one thing: renders a `.md` file in a window. No editor, no tabs, no sidebar, no plugins, no settings, no config files, no internet connection, no telemetry.

Double-click a markdown file. Read it. Close the window.

## Goals

- **Instant.** Opens faster than you can blink. Sub-100ms cold start.
- **Tiny.** Under 1MB binary. No runtime dependencies beyond what Windows 11 already has (WebView2).
- **Silent.** No update prompts, no splash screen, no first-run wizard, no notifications.
- **Stable.** One job, done correctly, forever.

## Non-Goals

- Editing
- Tabs
- File watching / live reload
- Themes or customization
- Mermaid / LaTeX / diagrams
- Export to PDF / Word / HTML
- Plugin system
- Cross-platform support

If you want those things, use [Tinta](https://tinta.cc), [Markpad](https://github.com/alecdotdev/Markpad), or [Typora](https://typora.io). They're good. ViewMD is for people who just want to read a file.

## Install

**Scoop:**
```
scoop bucket add viewmd https://github.com/FXGears/viewmd
scoop install viewmd
```

**Manual:** Download `viewmd.exe` from [Releases](https://github.com/FXGears/viewmd/releases) and put it anywhere.

To set as default for `.md` files:
1. Right-click any `.md` file → Open with → Choose another app
2. Select `viewmd.exe`
3. Check "Always use this app"

## Usage

```
viewmd.exe document.md
```

Or just double-click a `.md` file after setting the file association.

## Specs

- **Binary size:** 630 KB
- **Memory usage:** ~30 MB (WebView2 process)
- **Startup time:** <100ms
- **Dependencies:** None (WebView2 is pre-installed on Windows 10/11)
- **Written in:** Rust
- **Rendering:** WebView2 with GitHub-dark styling
- **License:** GPL-3.0

## Building

```
cargo build --release
```

Requires:
- Rust (stable)
- Visual Studio Build Tools (for MSVC linker)

Output: `target/release/viewmd.exe`
