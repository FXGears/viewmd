# Contributing to ViewMD

## The Rules

1. ViewMD reads markdown files. That's it. Don't propose features that go beyond that.
2. If your change adds more than 10KB to the binary, it needs a very good reason.
3. No runtime dependencies. No network access. No config files.
4. If you're not sure whether something belongs, open an issue first.

## Building

```
cargo build --release
```

Requires Rust (stable) and Visual Studio Build Tools.

## Pull Requests

- One change per PR
- Describe what and why
- Test with a real `.md` file before submitting

## Bugs

Open an issue. Include:
- What you opened (file size, any unusual markdown syntax)
- What you expected
- What happened instead
- Windows version
