# Contributing

This file is for people who build `savant` from source, publish a Windows release, or work on the protocol notes. Everyday install and programming stay in [README.md](README.md).

## Build from source

You need a stable Rust toolchain.

```bash
git clone https://github.com/SamLeatherdale/savant-elite.git
cd savant-elite
cargo build --release
```

Or install the binary with Cargo:

```bash
cargo install --path .
```

After `cargo build --release`, the Windows binary is `target\release\savant.exe`. You can copy it onto your `PATH`.

## Publish a Windows release

To publish a Windows ZIP without building on your machine, run the **Windows release** workflow from the Actions tab. Enter a tag such as `v0.4.0`. That run builds `savant.exe`, creates a GitHub Release for the tag, and attaches:

- `savant-x86_64-pc-windows-msvc.zip`
- `savant-x86_64-pc-windows-msvc.zip.sha256`

The workflow does not run on every push to `main`. Pushing an annotated `v*` tag starts the same job. There is no automated installer.

## Checks

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Hardware and script notes are in [TESTING.md](TESTING.md). Agent conventions are in [AGENTS.md](AGENTS.md).

## Completions

`savant completions` writes a clap completion script to stdout:

```bash
savant completions bash
savant completions zsh
savant completions fish
savant completions powershell
```

## macOS

macOS can build the CLI and run `status`, `info`, `monitor`, and `doctor`. Programming on macOS is unverified. There is no macOS installer. Grant Input Monitoring to the terminal before you run `savant monitor`.

## Protocol notes

- [RE_FINDINGS.md](RE_FINDINGS.md) — identity, transport, and protocol notes
- [docs/evidence/captures/MANIFEST.md](docs/evidence/captures/MANIFEST.md) — capture catalog
