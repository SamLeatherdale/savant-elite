# savant-elite

<div align="center">

<img src="https://raw.githubusercontent.com/SamLeatherdale/savant-elite/main/illustration.jpg" alt="Savant Elite Foot Pedal Illustration" width="480">

[![License: MIT](https://img.shields.io/badge/License-MIT%2BOpenAI%2FAnthropic%20Rider-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)

Windows command-line tool for the discontinued first-generation Kinesis Savant Elite / X-keys USB foot pedal.

This repository is a fork of [Dicklesworthstone/savant-elite](https://github.com/Dicklesworthstone/savant-elite). It targets the FS30A-12 and other first-generation models that use the original **learn** style of programming, not the protocol assumed upstream.

</div>

## Who this is for

`savant` programs first-generation Savant Elite units such as the **FS30A-12**, plus other models that share that learn-style programming path. Those units have a recessed Play / Program slide switch. In the original Kinesis software, you put the pedal in Program, tapped a pedal, typed keys on the keyboard, and tapped the pedal again so it learned the mapping.

The [upstream repository](https://github.com/Dicklesworthstone/savant-elite) used a different X-keys programming protocol than this hardware speaks. This fork writes mappings the way the FS30A-12 family actually accepts them.

This tool does **not** program Savant Elite2 units. Those use the SmartSet app and a virtual drive.

## Install savant

1. Open the [latest GitHub Release](https://github.com/SamLeatherdale/savant-elite/releases/latest).
2. Download `savant-x86_64-pc-windows-msvc.zip`.
3. Unzip it and copy `savant.exe` to a folder on your `PATH`.

To build from source or publish a release, see [CONTRIBUTING.md](CONTRIBUTING.md).

The first time you want to change a mapping, [install the programming driver once](#install-the-programming-driver-once). You do not repeat that step for later mappings.

## Program a key

`savant` shows whether the pedal is connected, watches what a pedal types in everyday use, and writes one mapping at a time.

1. Flip the pedal over. Use a pencil or paperclip to slide the recessed switch to **Program** (left). Unplug the USB cable, then plug it back in. The green LED should flash.
2. Run `savant status` and confirm it reports Programming mode.
3. Preview the mapping with no write to the pedal:

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

`--dry-run` does not write to the pedal. A real write requires `--yes`. There is no default write.

`savant preset` and `savant config` store names on disk only. To change a pedal mapping, use `savant program`.

## Install the programming driver once

Windows treats the pedal as two different devices:

- **Play** (switch to the right) is everyday use. Windows already knows how to talk to this device. Leave that driver alone.
- **Program** (switch to the left) is only for changing mappings. The first time Windows sees that device, you need to attach a programming driver.

This tool does not install USB drivers. Download [Zadig](https://zadig.akeo.ie/) and attach the programming driver yourself. You do this once per computer.

If you have never done this:

1. Download and run [Zadig](https://zadig.akeo.ie/).
2. Flip the recessed switch to **Program** (left). Unplug the USB cable, then plug it back in. The green LED should flash.
3. In Zadig, open **Options** and choose **List All Devices**.
4. Select **Footpedal**. Check **USB ID**:
   - `05F3 0232` means Program. Continue.
   - `05F3 030C` means Play. Do not replace that driver with WinUSB. Slide the switch to **Program** (left), unplug the USB cable, plug it back in, then pick **Footpedal** again.
5. Set the replacement driver to **WinUSB**, then choose **Replace Driver**.
6. Confirm with `savant status`. It should report Programming mode.

If you install WinUSB on the Play device (`030C`), `savant monitor` cannot read the pedal, and the pedals may stop typing keys in normal use.

## Play and Program

The unit has a recessed Play / Program slide switch on the underside. After you flip it, unplug the USB cable and plug it back in.

| Switch | Position | Green LED | What you can run |
| --- | --- | --- | --- |
| Play | Right | Steady | `status`, `info`, `monitor`, `doctor` |
| Program | Left | Flashing | `status`, `info`, `doctor`, `program`, `erase` |

## Commands

These flags apply to every `savant` command:

| Flag | Purpose |
| --- | --- |
| `-v`, `--verbose` | Extra detail on stderr |
| `--json` | Machine-readable output on commands that support it |
| `--timeout <MS>` | How long to wait for the pedal, 100–600000 (default 500) |

### `savant status`

Reports whether a Savant Elite is visible and whether it is in Play or Program mode.

### `savant info`

Lists matching connections for the pedal.

### `savant monitor`

Watches Play-mode pedal taps. On Windows, this follows the everyday Play device and ignores the rest of the keyboard.

```bash
savant monitor --duration 30
```

### `savant program`

Writes one mapping per run. Preview with `--dry-run`. A real write requires `--yes`.

Arguments:

- `--pedal` — which pedal to change: `a`, `b`, or `c`
- `--action` — what that pedal should send:
  - `clear` — remove that pedal's mapping
  - a single key, such as `a` or `right` (`right` is the Right Arrow key)
  - a single modifier with no key, such as `ctrl`
  - a mouse action: `left-click`, `right-click`, `middle-click`, `scroll-up`, or `scroll-down`
  - a chord of `[modifier+]key`, such as `ctrl+a`
  - a sequence of chords, separated by commas, such as `ctrl+a,b`
- `--dry-run` — print the mapping and do not write
- `--yes` — write the mapping to the pedal

Join modifiers and a key with `+`. Put more than one modifier in front of the same key, still separated by `+`:

- `ctrl+a` — Control and A together
- `ctrl+shift+a` — Control, Shift, and A together
- `ctrl+alt+delete` — Control, Alt, and Delete together
- `shift+alt+gui+s` — four modifiers and S

Order of the modifiers does not matter (`shift+ctrl+a` is the same as `ctrl+shift+a`). A comma starts a new tap, and later taps do not keep the earlier modifiers: `ctrl+a,b` is Control+A, then B by itself.

A modifier with no key is allowed (`ctrl`). Combinations of modifiers with no key, such as `shift+alt`, are not.

Left-side names: `ctrl`, `shift`, `alt`, `gui`. Right-side names: `rctrl`, `rshift`, `ralt`, `rgui`. Run `savant keys` for every accepted name.

Examples:

```bash
savant program --pedal a --action a --dry-run
savant --json program --pedal a --action ctrl+shift+a --dry-run
savant program --pedal c --action right --dry-run
savant program --pedal a --action left-click --dry-run
savant program --pedal a --action clear --dry-run
savant program --pedal a --action a --yes
```

After `--yes`, slide the switch to **Play** (right), unplug the USB cable, plug it back in, then watch the pedal. `savant monitor` shows keyboard keys. For mouse mappings, watch the pointer.

### `savant erase`

Clears every pedal mapping in one write.

Preview with `--dry-run`. A real erase requires `--yes`.

```bash
savant erase --dry-run
savant erase --yes
```

### `savant keys`

Lists the key names and modifier aliases you can pass to `--action`.

### `savant doctor`

Checks whether the computer can see the pedal and whether the programming driver is attached.

### `savant preset`

Lists or shows built-in mapping names. These commands do not write the pedal. To write a mapping, use `savant program`.

```bash
savant preset --list
savant preset zoom --show
```

Shipped names: `copy-paste`, `undo-redo`, `browser`, `zoom`.

### `savant config`

Saves, lists, and shows named profiles on disk. These commands do not write the pedal.

Replace `NAME` with the profile name:

```bash
savant config save NAME
savant config list
savant config show NAME
```

## History

Kinesis sold a USB foot controller built on PI Engineering X-keys hardware. The original Windows programmer did not survive on modern PCs. This tool is a replacement for first-generation learn-style units.

## License

MIT License (with OpenAI/Anthropic Rider). See [LICENSE](LICENSE).
