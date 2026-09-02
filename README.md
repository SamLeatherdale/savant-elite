# savant-elite

<div align="center">

<img src="https://raw.githubusercontent.com/Dicklesworthstone/savant-elite/main/savant_elite_illustration.webp" alt="Savant Elite Foot Pedal Illustration" width="600">

[![License: MIT](https://img.shields.io/badge/License-MIT%2BOpenAI%2FAnthropic%20Rider-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)

Windows-first Rust CLI for the discontinued Kinesis Savant Elite / X-keys USB foot pedal.

This repository is a fork of [Dicklesworthstone/savant-elite](https://github.com/Dicklesworthstone/savant-elite).

</div>

## Install and program a key

`savant` is a Windows command-line tool for a Kinesis Savant Elite / X-keys USB foot pedal. You use it to see whether the pedal is connected, watch Play-mode input, and write one key mapping at a time.

If you do not have a `savant` binary yet, [build from source or download a Windows release](#get-a-windows-build).

If you have not bound a programming driver yet, [bind WinUSB once](#bind-winusb-once) to Programming PID `05F3:0232` only. Never bind Play PID `05F3:030C`.

To write one mapping on Windows:

1. Flip the recessed switch to Program. Unplug the USB cable, then plug it back in.
2. Run `savant status` and confirm the CLI reports Programming mode (`05F3:0232`).
3. Preview the mapping with no USB write:

   ```bash
   savant program --pedal a --action a --dry-run
   ```

4. If the preview is the mapping you want, write it:

   ```bash
   savant program --pedal a --action a --yes
   ```

5. Flip the switch to Play. Unplug the USB cable, then plug it back in.
6. Watch the pedal:

   ```bash
   savant monitor
   ```

`--dry-run` does not open USB. A real write requires `--yes`. There is no default write.

Preset apply and `savant config load` do not write the pedal. To change a pedal mapping, use `savant program`.

## Get a Windows build

You need a stable Rust toolchain. Then you can build `savant` from this repository:

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

To publish a Windows ZIP without building on your machine, run the **Windows release** workflow from the Actions tab. Enter a tag such as `v0.3.0`. That run builds `savant.exe`, creates a GitHub Release for the tag, and attaches:

- `savant-x86_64-pc-windows-msvc.zip`
- `savant-x86_64-pc-windows-msvc.zip.sha256`

The workflow does not run on every push to `main`. Pushing an annotated `v*` tag starts the same job. There is no automated installer.

## Bind WinUSB once

Programming on Windows uses WinUSB on Programming PID `05F3:0232` only. Play PID `05F3:030C` must stay on the HID driver so `savant monitor` can read the pedal.

This repository does not install or rebind USB drivers. You bind the Programming interface yourself with Zadig or an equivalent tool.

If you have never bound the Programming interface:

1. Flip the recessed switch to Program. Unplug the USB cable, then plug it back in.
2. In Zadig, list all devices and select the Kinesis/X-keys Programming interface (`05F3:0232`).
3. Replace that driver with WinUSB. Bind Programming PID `05F3:0232` only. Never bind Play PID `05F3:030C`.
4. Confirm with `savant status` or `savant doctor`.

## Play and Program

The unit has a recessed Play / Program switch. After you flip it, unplug the USB cable and plug it back in.

This table shows which commands you can run in each mode:

| Mode | Product ID | What you can run |
| --- | --- | --- |
| Play | `05F3:030C` | `status`, `info`, `monitor`, `doctor` |
| Program | `05F3:0232` | `status`, `info`, `doctor`, `program` |

## Commands

These global flags apply to the `savant` binary:

| Flag | Purpose |
| --- | --- |
| `-v`, `--verbose` | Debug lines on stderr |
| `--json` | Machine-readable output on commands that support it |
| `--timeout <MS>` | USB timeout, 100–600000 (default 500) |

### `savant status`

Reports whether a Savant Elite is visible and whether the USB product ID looks like Play (`05F3:030C`) or Programming (`05F3:0232`).

### `savant info`

Lists matching HID and USB interfaces.

### `savant monitor`

Watches Play-mode pedal input. On Windows, this uses device-filtered Raw Input for PID `05F3:030C` and ignores the host keyboard.

```bash
savant monitor --duration 30
```

On macOS, grant Input Monitoring to the terminal before you run `savant monitor`. That path is observation only.

### `savant program`

Writes one mapping per invocation. Preview with `--dry-run` (no USB). A real write requires `--yes`.

```bash
savant program --pedal a --action a --dry-run
savant --json program --pedal a --action ctrl+a,b --dry-run
savant program --pedal c --action right --dry-run
savant program --pedal a --action clear --dry-run
savant program --pedal a --action a --yes
```

`--pedal` is `a`, `b`, or `c`. `--action` is `clear`, or `chord[,chord…]` where each chord is `[modifier+]key`. Left modifiers: `ctrl`, `shift`, `alt`, `gui` (and aliases). Right modifiers: `rctrl`, `rshift`, `ralt`, `rgui`. Run `savant keys` for the names the encoder accepts.

After `--yes`, switch the pedal to Play, unplug the USB cable, plug it back in, then watch the pedal with `savant monitor`.

### `savant keys`

Lists key names and modifier aliases the encoder accepts. `--json` is supported.

### `savant doctor`

Runs local diagnostics: binary, platform, device visibility, on-disk config, and Input Monitoring on macOS.

### `savant preset`

Lists or shows built-in mapping names. `--list` and `--show` do not write the pedal. Applying a preset does not write the pedal. To write a mapping, use `savant program`.

```bash
savant preset --list
savant preset zoom --show
```

Shipped names: `copy-paste`, `undo-redo`, `browser`, `zoom`.

### `savant config`

Saves, lists, and shows on-disk profiles. These commands do not write the pedal.

Replace `NAME` with the profile name:

```bash
savant config save NAME
savant config list
savant config show NAME
```

`savant config load` does not write the pedal. To write a mapping, use `savant program`.

### `savant completions`

Writes a clap completion script to stdout.

```bash
savant completions bash
savant completions zsh
savant completions fish
savant completions powershell
```

## macOS

macOS can build the CLI and run `status`, `info`, `monitor`, and `doctor`. Programming on macOS is unverified. There is no macOS installer.

## Technical notes

Install and everyday use stay in this README. Protocol notes and capture files are separate:

- [RE_FINDINGS.md](RE_FINDINGS.md) — identity, transport, and protocol notes
- [docs/evidence/captures/MANIFEST.md](docs/evidence/captures/MANIFEST.md) — capture catalog

## History

Kinesis sold a USB foot controller built on PI Engineering X-keys hardware (VID `0x05F3`). The original Windows programmer did not survive modern 32-bit-app removals. This fork keeps a native Rust CLI.

## License

MIT License (with OpenAI/Anthropic Rider). See [LICENSE](LICENSE).
