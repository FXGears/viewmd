# Changelog

## v0.9.1

- App icon: double-pane glass viewport, steel blue on dark navy
- Icon embedded in binary (title bar, taskbar, file explorer)
- Improved list item spacing for wrapped text
- Dark scrollbar styling
- Replaced `image` crate with `png` for smaller binary (814KB → 758KB)
- No console window on launch

## v0.9.0

Initial release.

Written in a single sitting to solve a problem: we had markdown files and no way to read them without launching an IDE or spinning up a local server. Every existing option was either an Electron app pretending to be lightweight, a note-taking tool that wanted to manage our files, or a browser extension asking for filesystem access.

We wanted the markdown equivalent of double-clicking a PDF. Open. Read. Close.

Rust, WebView2, pulldown-cmark, ~120 lines of code. Dark mode. GitHub-width content. No menu. No tabs. No settings.

---

## Design Principles

1. **If it doesn't serve reading, it doesn't ship.**
2. **The binary stays under 1MB.**
3. **Startup stays under 100ms.**
4. **Zero configuration.**
5. **No network access.**
