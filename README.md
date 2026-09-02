# savant-elite

<div align="center">

<img src="https://raw.githubusercontent.com/SamLeatherdale/savant-elite/main/illustration.jpg" alt="Savant Elite Foot Pedal Illustration" width="600">

[![License: MIT](https://img.shields.io/badge/License-MIT%2BOpenAI%2FAnthropic%20Rider-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)

Windows-first Rust CLI for the discontinued first-generation Kinesis Savant Elite / X-keys USB foot pedal.

This repository is a fork of [Dicklesworthstone/savant-elite](https://github.com/Dicklesworthstone/savant-elite). It targets the FS30A-12 and other first-generation models that use the original **learn** style of programming, not the protocol assumed upstream.

</div>

## Who this is for

`savant` programs first-generation Savant Elite units such as the **FS30A-12**, plus other models that share that learn-style programming path. Those units have a recessed Play / Program slide switch. In the original Kinesis software, you put the pedal in Program, tapped a pedal, typed keys on the keyboard, and tapped the pedal again so it learned the mapping.

The [upstream repository](https://github.com/Dicklesworthstone/savant-elite) used a different X-keys programming protocol than this hardware speaks. This fork writes mappings the way the FS30A-12 family actually accepts them.

This tool does **not** program Savant Elite2 units. Those use the SmartSet app and a virtual drive.

## Install and program a key

`savant` is a Windows command-line tool. You use it to see whether the pedal is connected, watch what a pedal types in everyday use, and write one key mapping at a time.

If you do not have a `savant` binary yet, [build from source or download a Windows release](#get-a-windows-build).

The first time you want to change a mapping, [install the programming driver once](#install-the-programming-driver-once). You do not repeat that step for later mappings.

To write one mapping on Windows:

1. Flip the pedal over. Use a pencil or paperclip to slide the recessed switch to **Program** (left). Unplug the USB cable, then plug it back in. The green LED should flash.
2. Run `savant status` and confirm it reports Programming mode.
3. Preview the mapping with no USB write:

   ```bash
   savant program --pedal a --action a --dry-run
   ```

4. If the preview is the mapping you want, write it:

   ```bash
   savant program --pedal a --action a --yes
   ```

5. Slide the switch to **Play** (right). Unplug the USB cable, then plug it back in. The green LED should glow steadily.
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

To publish a Windows ZIP without building on your machine, run the **Windows release** workflow from the Actions tab. Enter a tag such as `v0.4.0`. That run builds `savant.exe`, creates a GitHub Release for the tag, and attaches:

- `savant-x86_64-pc-windows-msvc.zip`
- `savant-x86_64-pc-windows-msvc.zip.sha256`

The workflow does not run on every push to `main`. Pushing an annotated `v*` tag starts the same job. There is no automated installer.

## Install the programming driver once

Windows treats the pedal as two different devices:

- **Play** (switch to the right) is everyday use. Windows already knows how to talk to this device. Leave that driver alone.
- **Program** (switch to the left) is only for changing mappings. The first time Windows sees that device, you need to attach a programming driver.

This repository does not install USB drivers. Download [Zadig](https://zadig.akeo.ie/) and attach the programming driver yourself. You do this once per computer.

If you have never done this:

1. Download and run [Zadig](https://zadig.akeo.ie/).
2. Flip the recessed switch to **Program** (left). Unplug the USB cable, then plug it back in. The green LED should flash.
3. In Zadig, open **Options** and choose **List All Devices**.
4. Select **Footpedal**. Check **USB ID**:
   - `05F3 0232` means Program. Continue.
   - `05F3 030C` means Play. Do not replace that driver with WinUSB. Slide the switch to **Program** (left), unplug the USB cable, plug it back in, then pick **Footpedal** again.
5. Set the replacement driver to **WinUSB**, then choose **Replace Driver**.
6. Confirm with `savant status` or `savant doctor`. It should report Programming mode.

If you install WinUSB on the Play device (`030C`), `savant monitor` cannot read the pedal, and the pedals may stop typing keys in normal use.

## Play and Program

The unit has a recessed Play / Program slide switch on the underside. After you flip it, unplug the USB cable and plug it back in.

| Switch | Position | Green LED | What you can run |
| --- | --- | --- | --- |
| Play | Right | Steady | `status`, `info`, `monitor`, `doctor` |
| Program | Left | Flashing | `status`, `info`, `doctor`, `program`, `erase` |

## Commands

These global flags apply to the `savant` binary:

| Flag | Purpose |
| --- | --- |
| `-v`, `--verbose` | Debug lines on stderr |
| `--json` | Machine-readable output on commands that support it |
| `--timeout <MS>` | USB timeout, 100–600000 (default 500) |

### `savant status`

Reports whether a Savant Elite is visible and whether it is in Play or Programming mode.

### `savant info`

Lists matching HID and USB interfaces.

### `savant monitor`

Watches Play-mode pedal input. On Windows, this follows the everyday Play device and ignores the host keyboard.

```bash
savant monitor --duration 30
```

On macOS, grant Input Monitoring to the terminal before you run `savant monitor`. That path is observation only.

### `savant program`

Writes one mapping per run. Preview with `--dry-run` (no USB). A real write requires `--yes`.

Arguments:

- `--pedal` — which pedal to change:
  - `a`
  - `b`
  - `c`
- `--action` — what that pedal should send:
  - `clear` — remove that pedal's mapping
  - a single key, such as `a` or `right` (`right` is the Right Arrow key)
  - a single modifier with no key, such as `ctrl`
  - a mouse action: `left-click`, `right-click`, `middle-click`, `scroll-up`, or `scroll-down`
  - a chord of `[modifier+]key`, such as `ctrl+a`
  - a sequence of chords, separated by commas, such as `ctrl+a,b`
- `--dry-run` — print the mapping and do not open USB
- `--yes` — write the mapping to the pedal

Modifiers you can put before a key:

- Left: `ctrl`, `shift`, `alt`, `gui` (and aliases)
- Right: `rctrl`, `rshift`, `ralt`, `rgui`

Run `savant keys` for the names the encoder accepts.

Examples:

```bash
savant program --pedal a --action a --dry-run
savant --json program --pedal a --action ctrl+a,b --dry-run
savant program --pedal c --action right --dry-run
savant program --pedal a --action left-click --dry-run
savant program --pedal a --action clear --dry-run
savant program --pedal a --action a --yes
```

After `--yes`, slide the switch to **Play** (right), unplug the USB cable, plug it back in, then watch the pedal. `savant monitor` shows keyboard keys. For mouse mappings, watch the pointer.

### `savant erase`

Clears every pedal mapping in one write. This is a different USB request from `program` and is device-wide, so it is not a `--pedal` / `--action` flag.

Preview with `--dry-run` (no USB). A real erase requires `--yes`.

```bash
savant erase --dry-run
savant erase --yes
```

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

Kinesis sold a USB foot controller built on PI Engineering X-keys hardware. The original Windows programmer did not survive modern 32-bit-app removals. This fork keeps a native Rust CLI for the first-generation learn-style units.

## License

MIT License (with OpenAI/Anthropic Rider). See [LICENSE](LICENSE).
