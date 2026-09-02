# Protocol findings

Canonical notes for the Kinesis Savant Elite / X-keys unit in this repository. Not a universal X-keys specification. For install and everyday commands, see [README.md](README.md).

**Evidence:** the seven USBPcap files in [docs/evidence/captures/](docs/evidence/captures/) and the raw catalog [docs/evidence/captures/MANIFEST.md](docs/evidence/captures/MANIFEST.md). Frame numbers, times, and `usb.data_fragment` bytes live only in that catalog.

Every protocol claim carries or inherits one class. These six classes apply to the USBPcap catalog only:

| Class | Meaning |
| --- | --- |
| **Verified** | Setup fields and payload bytes are in the catalog, and the operator label matches those tokens. |
| **Observed** | Bytes or transfers are in the traces without a matching verified label or a proven field meaning. |
| **Inferred** | Pattern comparison across verified payloads. Not a captured named field. |
| **Family-other** | USB HID spec, other X-keys units, or upstream family docs. Not evidenced on this captured unit. |
| **Contradicted** | Upstream or family claim that these traces disprove **for this captured unit**. Not a claim that no other unit ever behaved that way. |
| **Unknown** | No evidence in this archive. |

Unlabeled sentences inherit the nearest section default. Play-mode HID modifier bits are never Programming-mode `F0`–`F7` / `FE` tokens.

Operator/runtime Play checks (native request-6 write + Windows Raw Input) are a separate evidence stream. They never reclassify catalog frames and are not capture **Verified** bytes.

Do not treat mouse, media, delays, repeats, max macro length, standalone LGUI+A, keypad digits, or keypad decimal with NumLock on as fact. A capture no-write is not a device rejection. True F13–F24 (`68`–`73`) failed native Play tests and are unsupported; keep those HID values as spec context only. The CLI encoder covers captured Keyboard/Keypad short chords, not a general X-keys family.

## Identity

**Default:** Verified for USB IDs from the seven pcaps.

| Field | Value | Confidence |
| --- | --- | --- |
| Vendor ID | `0x05F3` | Verified |
| Product ID during the programming writes | `0x0232` | Verified |
| `bcdDevice` | `0x0100` | Observed |
| `bcdUSB` | `0x0110` | Observed |
| Device class / subclass / protocol | `0x00` / `0` / `0` | Observed |
| Configurations | 1 | Observed |
| `iSerialNumber` | `0` | Observed |
| Play PID `0x030C` | not in these traces | Family-other / Unknown here |
| Physical Play / Program switch | operator procedure | Family-other |

All seven traces enumerate only `05F3:0232` on endpoint `4.1.0`. They contain no Play-mode product ID and no HID interrupt reports. Verified pedal selectors are **A** (`01`), **B** (`02`), and **C** (`03`). Kinesis branding on PI Engineering hardware (`0x05F3`) is Family-other.

## Request 6 vs request 7

**Default:** Verified for the request-6 write envelope.

Every successful programming write is a **host-to-device vendor endpoint** control transfer:

| Field | Value |
| --- | --- |
| Direction / type / recipient | Host-to-device, Vendor, Endpoint |
| `bmRequestType` | `0x42` |
| `bRequest` | `6` |
| `wValue` | `0` |
| `wIndex` | `0` |
| USBPcap destination | host → `4.1.0` (control endpoint 0) |
| `wLength` | 5, 8, 11, 13, 14, 16, 17, or 20 (equals payload length) |

Clean verified rows use 8, 11, 14, 17, or 20. Length 5 is the labelled clear. Length 13 is only the malformed intended-LGUI+A row (**Observed**). USBPcap records `URB_FUNCTION_VENDOR_ENDPOINT` on the setup stage. Complete-stage URBs for these writes carry no IN data. There is no second “save” transfer in the request-6 set. This is the only verified programming write.

### Request 7 (do not over-call this “status”)

**Observed** in all seven pcaps. Not a decoded status register.

| Field | Value | Confidence |
| --- | --- | --- |
| `bmRequestType` | `0xC2` (device-to-host, vendor, endpoint) | Observed |
| `bRequest` | `7` | Observed |
| `wValue` / `wIndex` | `0` / `0` | Observed |
| `wLength` | `7` | Observed |

The host polls this IN throughout each session. Complete-stage 7-byte payloads seen in the original four files (counts mix request-3 completions; see below):

| Response | Golden | Held | Hooked | UAC-off |
| --- | ---: | ---: | ---: | ---: |
| `00 89 00 00 00 00 00` | 2301 | 897 | 716 | 1414 |
| `00 89 05 00 00 00 00` | 262 | 96 | 5 | 3 |
| `00 89 06 00 00 00 00` | 62 | 0 | 0 | 0 |
| `00 08 01 00 01 00 00` | 5 | 2 | 1 | 1 |

Golden pairing: request-7 setup (frame 7) completes with `00 89 00 00 00 00 00` (frame 8). `00 08 01 00 01 00 00` pairs with **request 3**, not request 7 (frames 2649–2650, immediately before a request-6 write). Field meanings are **Unknown**. `savant status` today reports USB PID only; it does not issue request 7.

Other vendor transfers in the same traces (**Observed**, purpose **Unknown**):

| Envelope | Notes |
| --- | --- |
| `0x42` / `bRequest=2` / `wLength=1` | OUT payloads alternate `00` and `02` |
| `0xC2` / `bRequest=3` / `wLength=7` | In golden, one setup immediately before each of the five request-6 writes; complete data `00 08 01 00 01 00 00` |

These are not programming writes and are not part of a verified encoder.

## Verified vectors

**Default:** Verified for named rows. Bytes are `usb.data_fragment`. Every row used the request-6 envelope above. Frame-level catalog: [MANIFEST.md](docs/evidence/captures/MANIFEST.md).

Original six:

| Scenario | Pedal | Payload |
| --- | --- | --- |
| Pedal A→a | A | `01 00 00 01 02 04 fe 04` |
| Pedal A→b | A | `01 00 00 01 02 05 fe 05` |
| Pedal B→a | B | `02 00 00 01 02 04 fe 04` |
| Ctrl+A | A | `01 00 00 02 04 f0 04 fe 04 fe f0` |
| a then b | A | `01 00 00 06 00 04 fe 04 05 fe 05` |
| Ctrl+A then b | A | `01 00 00 09 00 f0 04 fe 04 fe f0 05 fe 05` |

Pedal A→a also appears as hooked frame 1303 and UAC-off frame 2023 (same bytes as held 1255). Held frame 561 (`01 00 00 06 00 04 fe 04 04 fe 04`) is **Observed** only — not a named scenario.

`encode_program` still matches those exact strings, and now also encodes Pedals A–C, tokens `F0`–`F7`, captured `clear`, comma-separated short chords via the recovered header rule, and supported Keyboard/Keypad names. It rejects consumer/media/mouse/power/delay/repeat/numeric usages and true `F13`–`F24` before USB (`savant program --action f13` is a rejection), instead of falling back to `0xCC` or `SET_REPORT`. `savant program` sends one request-6 transfer of that payload.

**Operator / runtime (not a capture).** Pedal A→a is hardware-verified (2026-09-02): after that write, Pedal A typed lowercase `a` in Play mode. Later native writes on the same Windows host confirmed more Keyboard/Keypad categories and a Pedal C `clear` disable. Inventory and leftover lab state: [Operator / runtime Play inventory](#operator--runtime-play-inventory). Those confirmations are not USBPcap Play-mode reports. All `F0`–`F7` have capture evidence; not every token had an independent native-CLI Play test.

### Extended tokens (bd-pka.10)

Payload bytes and times are in the catalog. Summary only.

**Pedal C.** Verified selector `03` on Pedal C→a (`03 00 00 01 02 04 fe 04`).

**HID usages (Verified taps).** `1E` Keyboard 1; `38` `/`; `28` Enter; `3A` F1; `45` F12; `4C` Delete Forward; `4F` Right Arrow; `58` Keypad Enter; `04`/`05`/`06` on a,b,c.

**Modifier tokens F0–F7** are capture-supported overall. Cite these clean rows:

| Token | Role | Clean evidence |
| --- | --- | --- |
| `F0` | Left Ctrl | Original Ctrl+A; Ctrl+Shift+A; Ctrl+Alt+A; Ctrl+Shift+Alt+A; Ctrl+Shift+Alt+GUI+A |
| `F1` | Left Shift | LShift+A; Shift+A then b; the Shift-containing combos above |
| `F2` | Left Alt | LAlt+A; Ctrl+Alt+A; Shift+Alt+A; Ctrl+Shift+Alt+A; Ctrl+Shift+Alt+GUI+A |
| `F3` | Left GUI | **Not** a standalone LGUI+A. Confirmed inside Ctrl+Shift+Alt+GUI+A (`… f0 f1 f2 f3 04 fe 04 fe f3 …`). Also present on Observed GUI-sequence rows |
| `F4` | Right Ctrl | RCtrl+A in the sequences capture (`… f4 04 fe 04 fe f4`) |
| `F5` | Right Shift | RShift+A |
| `F6` | Right Alt | RAlt+A |
| `F7` | Right GUI | RGUI+A. An earlier intended-RCtrl+A write used the same `F7` bytes (**Observed** / host remap; not RCtrl evidence) |

**Sequences / clear.** Verified: a,b,c; Shift+A then b. **Observed / labelled:** Escape×3 clear `01 00 00 00 00`. **Observed (not Ctrl):** three intended-Ctrl sequence rows arrived with `F3` (GUI-sequence evidence).

**Capture no-write (not a rejection).** The legacy host sent no request-6 for F13, F14, F15, F24, PrintScreen, ScrollLock, Pause, or Application/Menu. That cannot distinguish host interception from device rejection.

**Native writes (operator/runtime).** The current encoder later wrote those keys. PrintScreen (`46`), ScrollLock (`47`), Pause (`48`), Application (`65`), and navigation/keypad names encode. Play confirmation now covers those special keys, the navigation cluster, keypad operators, selector `03`, modifier `F4`, and captured `clear` — see [Operator / runtime Play inventory](#operator--runtime-play-inventory). True F13–F24 (`68`–`73`) wrote successfully (8 or 11 bytes) but produced no Play events; they are unsupported. The CLI now rejects `F13`–`F24` before USB. Standard HID `0x68`–`0x73` remain spec context only. macOS may present PC PrintScreen/ScrollLock/Pause as F13/F14/F15; those host names are usages `46`/`47`/`48`, not HID F13–F15.

Standalone intended LGUI+A produced a malformed 13-byte row, not a clean `F3` wrap.

## Operator / runtime Play inventory

**Not USBPcap.** Native `savant program` request-6 writes on this Windows host, then `savant monitor` device-filtered Raw Input for Play PID `05F3:030C`. These rows do not add frames to the capture archive (interrupt-IN count remains 0; no PID `0x030C` traffic in the seven pcaps). Do not cite them as catalog **Verified** bytes.

**Initial confirmations**

| Mapping | Write | Play result |
| --- | --- | --- |
| Pedal A→a | request-6 | lowercase `a` |
| Pedal C→Right Arrow | usage `4F` | Right press/release |
| Pedal B→Pause | usage `48` | Pause press/release |
| RCtrl+F24 | 11-byte write | only Ctrl (`F4`); no F24 |
| True F13 / F24 | 8-byte writes accepted | zero Play events |
| `F13`–`F24` in the CLI | rejected before USB | not sent |

**Later edge-case groups** (same native write + Raw Input method):

| Group | Mapping | Usage | Play result |
| --- | --- | --- | --- |
| 1 | PrintScreen | `46` | RELEASE only (`VK_SNAPSHOT`); expected Windows behavior |
| 1 | ScrollLock | `47` | press/release |
| 1 | NumLock | `53` | press/release |
| 2 | Application/Menu | `65` | press/release |
| 2 | KeypadDivide | `54` | press/release; distinct from slash `38` |
| 2 | Insert | `49` | press/release |
| 3 | Home | `4A` | press/release |
| 3 | End | `4D` | press/release |
| 3 | PageDown | `4E` | press/release |
| 4 | PageUp | `4B` | press/release |
| 4 | Left | `50` | press/release |
| 4 | Up | `52` | press/release |
| 5 | Down | `51` | press/release |
| 5 | KeypadMultiply | `55` | press/release |
| 5 | KeypadPlus | `57` | press/release |
| 6 | KeypadSubtract | `56` | press/release |
| 6 | KeypadDecimal | `63` | emitted Delete while NumLock off (normal keypad ./Del translation, not a drop) |
| 6 | Pedal C `clear` | `03 00 00 00 00` | no events after C tap; disable confirmed |

**Current leftover lab state:** Pedal A Keypad Subtract, Pedal B Keypad Decimal, Pedal C cleared. That is leftover lab state, not a recommended everyday mapping.

**Still unconfirmed here:** keypad decimal with NumLock on; keypad digits; standalone native LGUI and multi-mod release semantics; mouse, media, delay, repeat.

## Inferred layout

**Default:** Inferred. Not captured field names. The CLI encoder uses the recovered N/M header for short Keyboard/Keypad chords; do not invent a broader family rule.

| Offset | Pattern | Inference | Do not conclude |
| --- | --- | --- | --- |
| 0 | `01` Pedal A; `02` Pedal B; `03` Pedal C | Pedal select | Other numbering schemes |
| 1–2 | `00 00` on every request-6 payload here, including clear | Unused or fixed on this unit | Reserved-must-be-zero as a family rule |
| 3–4 | `01 02` single tap; `02 04` one modifier; `03 06` / `04 08` / `05 0a` two–four modifiers; `06 00` / `09 00` / `0c 00` / `0f 00` sequences; `00 00` clear; `07 04` and `09 ff` on Observed rows only | Recovered header: `N==1` → `00`, `M+1`, `2*(M+1)`; `N>=2` → body length as 16-bit BE then `00` | A family rule beyond these short Keyboard/Keypad chords |
| 5+ | HID usage in `KEY FE KEY`; modifiers wrap as `Fn … FE Fn` | Keyboard usage IDs and F0–F7 tokens | Mouse, media, hold/repeat, or programmable F13–F24 |

Worked readings (verified rows only): Pedal A→a is select `01` then tap `04 FE 04`. Pedal C→a is the same tap after `03`. Ctrl+A is `F0`, tap `04`, `F0`. LShift+A is `F1`, tap `04`, `F1`. a,b,c is tap `04` then `05` then `06`. Shift+A then b is that Shift+A run then tap `05`. Ctrl+Shift+Alt+GUI+A opens `F0 F1 F2 F3`, taps `04` once, and closes with the captured `FE` token order. Wrap-close order is Observed per row, not a proven family rule.

How the device commits the write is **Unknown**. The traces show one request-6 OUT per named scenario. “Written to EEPROM by `0xCE`” is **Contradicted** for this unit.

## Contradicted upstream assumptions

Wrong as the programming path **on this captured unit**. Not a statement about every X-keys product.

| Upstream claim | This unit |
| --- | --- |
| HID `SET_REPORT` (`bmRequestType 0x21`, `bRequest 0x09`) | Count is 0 in the seven pcaps |
| `CMD_SET_KEY_MACRO` `0xCC` | No request-6 payload starts with `CC` |
| `CMD_GET_KEY_MACRO` `0xCD` readback | No `CD` programming transfer |
| `CMD_SAVE_TO_EEPROM` `0xCE` as a separate save | No separate save |
| 36-byte HID output / feature reports | Request-6 lengths are 5–20 bytes as catalogued |
| Format spray (`fmt1-feat` → 36-byte → vendor `0x40`) | Single `0x42` / request 6 envelope |
| Pedal indices 0/1/2 inside an `0xCC` buffer | Byte 0 is `01` (A), `02` (B), or `03` (C) |
| Programming-mode modifiers are the HID bitmap | Tokens `F0`–`F7`; HID bits are Play-mode only |
| Current `savant program` writes a working mapping | Letters (A→a), navigation, locks, Pause, Application/Menu, keypad operators, captured `clear`, and RCtrl (`F4` in RCtrl+F24) are operator-confirmed in Play mode. True F13–F24 write-then-silent results are a device limitation. Keypad digits and keypad decimal with NumLock on remain without a Play check |
| macOS programming is verified | Unknown here (traces are from a Windows 7 lab host) |

Family command bytes such as `0xB5`, `0xB6`, `0xC1`, `0xCA` and a 36-byte SDK template are **Family-other**. Do not import them as this unit’s protocol.

Retained as CLI or product context, not protocol proof: VID `0x05F3`, Programming PID `0x0232`, Play-mode HID monitor with standard boot-keyboard bits, and the physical switch as operator procedure. Play PID `0x030C` stays Family-other until a Play-mode capture exists.

## Play mode

**Default:** Family-other for USB HID keyboard semantics. These seven pcaps have **no** Play-mode traffic (interrupt-IN count 0; no PID `0x030C`).

A successful request-6 URB is not Play-mode confirmation. Scenario names are Verified as **labels** in the catalog; the confirmation reports themselves are not stored here.

**Operator confirmation (not a capture):** on 2026-09-02, after the native Pedal A→a request-6 write on this Windows host, Pedal A typed lowercase `a` in Play mode. Later native writes on the same host confirmed the Keyboard/Keypad inventory in [Operator / runtime Play inventory](#operator--runtime-play-inventory). Those are operator/runtime results. They are not USBPcap Play-mode HID reports and do not change the archive (interrupt-IN count remains 0; no PID `0x030C` traffic). Keep captured Programming-mode bytes distinct from these confirmations.

macOS may translate PC PrintScreen/ScrollLock/Pause to F13/F14/F15 in the host UI. Those names still refer to usages `46`/`47`/`48`, not true HID F13–F15 (`68`–`6A`). Leftover lab state is Pedal A Keypad Subtract, Pedal B Keypad Decimal, Pedal C cleared; that is not a recommended everyday mapping.

If a Play-mode monitor parses boot keyboard reports, use **standard HID modifier bits** (Family-other, HID Usage Tables):

| Bit | Modifier |
| ---: | --- |
| `0x01` | Left Control |
| `0x02` | Left Shift |
| `0x04` | Left Alt |
| `0x08` | Left GUI |
| `0x10` / `0x20` / `0x40` / `0x80` | Right Ctrl / Shift / Alt / GUI |

Usage `0x04` is Keyboard A and `0x05` is Keyboard B in that spec. Those same usage numbers appear inside Programming-mode payloads; the Programming-mode **wrapper** is `KEY FE KEY` and optional `F0`–`F7`, not this bitmap.

`savant monitor` observes Play mode only. macOS uses hidapi on usage page `0x0001` / usage `0x0006` and decodes boot-style reports. Windows uses device-filtered Raw Input (`RAWKEYBOARD`) for `VID_05F3&PID_030C` and does not treat those fields as 8-byte HID reports. That is a Play-mode observer, not a verified programming path. Factory-default chords such as Ctrl+Alt+4 are Family-other and not in this archive.

## Capture scope and privacy

The seven traces sit in Git next to the catalog. Hashes and frames: [MANIFEST.md](docs/evidence/captures/MANIFEST.md).

| File | Role |
| --- | --- |
| `xkeys-golden-8f130110-0f9b-4d2b-b699-0477053b768a.pcap` | Five original verified scenarios |
| `xkeys-held-2b2e6f50-b917-4c0d-803d-f95e283debce.pcap` | Pedal A→a; one unlabeled request-6 |
| `xkeys-hooked-e44cc6e6-abf3-418e-a8b4-04b5df69e97e.pcap` | Pedal A→a (same payload as held 1255) |
| `xkeys-uac-off-9bc7213e-470f-43cd-9e0c-7e064e5f5300.pcap` | Pedal A→a (same payload as held 1255) |
| `xkeys-extended-keys-d7954dca-9d3a-43ec-8ff4-7e6cbe8919c4.pcap` | Extended HID usages; Observed `I fo` row; F13–F24 / special-key no-writes |
| `xkeys-extended-modifiers-9832c176-c560-4dbb-b423-93afdf28edb0.pcap` | F0–F7 chords; malformed LGUI; intended RCtrl as `F7` |
| `xkeys-extended-sequences-8b4e33be-a9f6-4bfa-a335-0237b2047aee.pcap` | `F4`; selector `03`; sequences; Observed GUI rows; labelled clear |

Provenance: recovered on 2026-09-02 from a Windows 7 lab programming session. SHA-256 is of the stored file bytes. Extracted with Wireshark `tshark` 4.6.8. Shared docs record repository-relative names only.

These files are raw USBPcap of Programming-mode sessions. All seven contain only endpoint `4.1.0`, identified as VID `05F3` PID `0232`; they have no captured USB serial-number strings and no other device endpoints. They still expose raw pedal control traffic, timing, poll responses, and programmed macro payloads, but not arbitrary host keyboard text. They are not Play-mode keystroke logs and are not a mapping readback. Do not treat a successful URB as proof the mapping persisted.

Three other lab traces were inspected and **not** stored here (no `0x42` / request-6 frames): `xkeys-programming-capture.pcap`, `xkeys-programming.pcap`, `xkeys-scheduled-8c2877b8-e98a-4f09-80b7-93aa513ff8ee.pcap`. Hashes are in the catalog.

## Open questions

**Default:** Unknown. Do not close by analogy.

Closed on this unit (operator/runtime, not capture frames): PrintScreen (`46`, RELEASE-only — expected Windows), ScrollLock, Application/Menu, captured `clear` (Pedal C disable), and true F13–F24 (unsupported). Still open: standalone native LGUI and multi-mod release semantics, keypad decimal with NumLock on, keypad digits, mouse, media, delays, repeats, long macros, and maximum length.

| Question | Why it is open |
| --- | --- |
| Meaning of bytes 1–2 (`00 00`) | Observed constant, including Pedal C and clear; not varied |
| Meaning of bytes 3–4 | Recovered N/M header covers clean short chords; Observed `07 04` / `09 ff` still sit outside that |
| Whether `FE` is release, terminator, or something else | Only `KEY FE KEY` and wrap `FE Fn` are seen |
| Hold vs tap | No verified hold (held frame 561 is unlabeled) |
| Standalone LGUI+A / multi-mod release semantics | Intended standalone LGUI+A was malformed/intercepted; `F3` is capture-confirmed only inside Ctrl+Shift+Alt+GUI+A and on Observed GUI-sequence rows. No native Play test of standalone LGUI or multi-mod release order |
| Request 7 field decode | 7-byte IN values vary; no named fields |
| Purpose of request 2 and request 3 | Observed only |
| Whether a successful URB equals a persisted mapping | No Play-mode reports in this archive. Operator confirmation exists for the [runtime inventory](#operator--runtime-play-inventory); those are not captured reports |
| F13–F24 programmability | Closed for this unit: native 8-byte / 11-byte writes produced no Play events; the CLI rejects them before USB. HID `0x68`–`0x73` stay spec context only |
| PrintScreen, ScrollLock, Application/Menu, `clear` | Closed: native Play confirmed PrintScreen RELEASE-only, ScrollLock press/release, Application press/release, and Pedal C `clear` disable |
| Keypad decimal with NumLock on | Optional gap: with NumLock off the `63` write emitted Delete (normal ./Del translation). No NumLock-on retest |
| Keypad digits | Encode; no native Play check |
| Mouse, media, delay, repeat | No vectors |
| Any persist mechanism other than the single request-6 write | `0xCE` / `0xCD` contradicted here; otherwise Unknown |
| macOS `0x42` / request 6 | Not tested in this archive |
| Play PID `0x030C` on this unit | Not enumerated here |
| Linux | Unknown |

Catalog any new capture in [MANIFEST.md](docs/evidence/captures/MANIFEST.md) with a confidence class before documenting a new token.
