# Changelog

## v0.9.1

- App icon: double-pane glass viewport, steel blue on dark navy
- Icon embedded in binary (title bar, taskbar, file explorer)
- Improved list item spacing and line-height for wrapped text
- Cleaned up development artifacts from repo

## v0.9.0

Initial release.

ViewMD was written in a single sitting to solve a problem: we had a pile of markdown files and no way to read them without launching an IDE or spinning up a local server. Every existing option was either an Electron app pretending to be lightweight, a note-taking tool that wanted to manage our files, or a browser extension asking for access to the local filesystem.

We wanted the markdown equivalent of double-clicking a PDF. Open it. Read it. Close it. Nothing else.

So we one-shotted it into existence. Rust, WebView2, pulldown-cmark, 120 lines of code, 630KB binary. Dark mode because we stare at screens all day. GitHub-width content because that's what readable looks like. No menu because there's nothing to configure. No tabs because it opens one file. No settings because there are no decisions to make.

It worked. We used it. Then we gave it a name, gave it an icon, and put it here in case anyone else is tired of the bloat.

---

## Design Principles (for contributors and future us)

1. **If it doesn't serve reading, it doesn't ship.** No editor. No export. No diagrams. No plugins.
2. **The binary stays under 1MB.** Every dependency is a liability.
3. **Startup stays under 100ms.** If you can perceive the app loading, something is wrong.
4. **Zero configuration.** No config files, no settings dialog, no first-run experience.
5. **No network access.** ViewMD never phones home, never checks for updates, never loads remote resources.
