# Capture catalog (bd-pka.1 / bd-pka.10)

USBPcap traces of vendor programming writes on the Kinesis Savant Elite / X-keys unit. This file is the **raw capture catalog**: filenames, SHA-256, sizes, tshark setup fields, `usb.data_fragment` bytes, and operator-labeled scenarios.

Field meanings are protocol interpretation. See [RE_FINDINGS.md](../../../RE_FINDINGS.md).

Recovered on 2026-09-02 from a Windows 7 lab programming session. SHA-256 is of the stored file bytes. Frames were extracted with Wireshark `tshark` 4.6.8:

```text
tshark -r <file.pcap> -Y "usb.bmRequestType == 0x42 && usb.setup.bRequest == 6" \
  -T fields -e frame.number -e frame.time_relative -e usb.dst \
  -e usb.bmRequestType -e usb.setup.bRequest -e usb.setup.wValue \
  -e usb.setup.wIndex -e usb.setup.wLength -e usb.data_fragment
```

Every listed request-6 frame is host -> `4.1.0`, `bmRequestType=0x42`, `bRequest=6`, `wValue=0`, `wIndex=0`. `Time (s)` is `frame.time_relative`. `wLength` equals the payload length.

## Capture evidence vs inferred protocol

| Kind | What this catalog records |
| --- | --- |
| Capture evidence | Filename, SHA-256, repository-relative path, tshark setup fields, `usb.data_fragment` bytes, operator label, and class |
| Inferred protocol | Documented in [RE_FINDINGS.md](../../../RE_FINDINGS.md). Not captured as named fields. Do not use as extra scenarios. |

| Class | Meaning in this catalog |
| --- | --- |
| **Verified** | Request-6 bytes are present and the operator label matches those tokens |
| **Observed** | Request-6 bytes are present; the operator intent is not used as the token name |
| **No-write** | Operator attempted an action; no request-6 frame exists. Not a device-rejection proof |

Original verified labels: Pedal A->a, Pedal A->b, Pedal B->a, Ctrl+A, a then b, Ctrl+A then b. Extended-session labels are in the tables below.

A no-write row means the host sent no `0x42` / request 6. That cannot distinguish host interception, legacy-UI absence, or a device rejection.

## Files

Stored in this directory (`docs/evidence/captures/`).

| Filename | SHA-256 | Bytes | Scenario coverage |
| --- | --- | --- | --- |
| `xkeys-golden-8f130110-0f9b-4d2b-b699-0477053b768a.pcap` | `757b0028ec34a0a8ce21df2f30e8bc62dead51c4fd73ec3247ca89f3d7d0cfe9` | 308403 | Pedal A->b; Pedal B->a; Ctrl+A; a then b; Ctrl+A then b |
| `xkeys-held-2b2e6f50-b917-4c0d-803d-f95e283debce.pcap` | `b55b3401a1c8fe47c52407e4b6701c5b9532f20d25c2332e0efbc470e6bdcaaa` | 116340 | Pedal A->a (frame 1255). Frame 561 payload is captured but unlabeled |
| `xkeys-hooked-e44cc6e6-abf3-418e-a8b4-04b5df69e97e.pcap` | `566566c8cf3138f9f147e5c2e63ff756118b53396590a0b4d218abe2b0d5da3f` | 83554 | Pedal A->a |
| `xkeys-uac-off-9bc7213e-470f-43cd-9e0c-7e064e5f5300.pcap` | `7235a2f08ed1e687544e6e258b70f5b4c1613400159314e187f40380c5d22abf` | 162421 | Pedal A->a |
| `xkeys-extended-keys-d7954dca-9d3a-43ec-8ff4-7e6cbe8919c4.pcap` | `3abf7ea072c640d89f018d476736ac7639a0f3935c58953dc111aef3adf85c56` | 1023226 | HID usage taps; 20-byte Observed row; F13–F24 / special-key no-writes |
| `xkeys-extended-modifiers-9832c176-c560-4dbb-b423-93afdf28edb0.pcap` | `5aaad3b46b5a47d6994d89856f26fe74e4fe6a4afab2dbfe1bf3a993547a0d82` | 695306 | F0–F7 chords; malformed intended LGUI+A; intended RCtrl+A as F7 |
| `xkeys-extended-sequences-8b4e33be-a9f6-4bfa-a335-0237b2047aee.pcap` | `c8ecc46a92467050bed5fe53e8cd7e8c860a2671e4f497fcf8b61e3ae5df464b` | 919162 | RCtrl+A (F4); Pedal C->a; a,b,c; Shift+A then b; F3 sequence rows; clear |
| `xkeys-mouse-advanced-7c2f9a11-4e60-4b8d-9c1a-restart2.pcap` | `15e856f250ba8e169a612340dceecf19b8d2743af17655e5fdf3649fe21bf033` | 851415 | Mouse clicks and self-scroll; intended delay / repeat / press-release; modifier-only Ctrl; device-wide erase as request 8 |

Hooked and UAC-off are extra successful programming captures: each contains the same Pedal A->a request-6 payload as held frame 1255.

All eight traces enumerate only `05F3:0232` on `4.1.0` (`bcdDevice=0x0100`, `bcdUSB=0x0110`, `iSerialNumber=0`). Interrupt-IN count is 0; Play PID `0x030C` count is 0; HID `SET_REPORT` (`0x21`/`9`) count is 0.

## Extracted request-6 vectors

Payloads are `usb.data_fragment` as captured.

### `xkeys-golden-8f130110-0f9b-4d2b-b699-0477053b768a.pcap`

| Frame | wLength | Payload | Scenario |
| ---: | ---: | --- | --- |
| 2651 | 8 | `01 00 00 01 02 05 fe 05` | Pedal A->b |
| 3275 | 8 | `02 00 00 01 02 04 fe 04` | Pedal B->a |
| 4141 | 11 | `01 00 00 02 04 f0 04 fe 04 fe f0` | Ctrl+A |
| 4997 | 11 | `01 00 00 06 00 04 fe 04 05 fe 05` | a then b |
| 5885 | 14 | `01 00 00 09 00 f0 04 fe 04 fe f0 05 fe 05` | Ctrl+A then b |

### `xkeys-held-2b2e6f50-b917-4c0d-803d-f95e283debce.pcap`

| Frame | wLength | Payload | Scenario |
| ---: | ---: | --- | --- |
| 561 | 11 | `01 00 00 06 00 04 fe 04 04 fe 04` | Unlabeled captured payload (not one of the verified scenario names) |
| 1255 | 8 | `01 00 00 01 02 04 fe 04` | Pedal A->a |

### `xkeys-hooked-e44cc6e6-abf3-418e-a8b4-04b5df69e97e.pcap`

| Frame | wLength | Payload | Scenario |
| ---: | ---: | --- | --- |
| 1303 | 8 | `01 00 00 01 02 04 fe 04` | Pedal A->a |

### `xkeys-uac-off-9bc7213e-470f-43cd-9e0c-7e064e5f5300.pcap`

| Frame | wLength | Payload | Scenario |
| ---: | ---: | --- | --- |
| 2023 | 8 | `01 00 00 01 02 04 fe 04` | Pedal A->a |

### `xkeys-extended-keys-d7954dca-9d3a-43ec-8ff4-7e6cbe8919c4.pcap`

Nine request-6 writes, in capture order. After F12 the operator attempted F13, PrintScreen, F14, ScrollLock, F15, Pause, and F24 (no-write). After Keypad Enter: Application/Menu (no-write).

| Frame | Time (s) | wLength | Payload | Classification |
| ---: | ---: | ---: | --- | --- |
| 2453 | 60.562500 | 8 | `01 00 00 01 02 1e fe 1e` | Verified: Keyboard 1 (usage `1E`) |
| 2911 | 70.812500 | 8 | `01 00 00 01 02 38 fe 38` | Verified: Keyboard / (usage `38`) |
| 3201 | 77.312500 | 8 | `01 00 00 01 02 28 fe 28` | Verified: Enter (usage `28`) |
| 3713 | 88.765625 | 8 | `01 00 00 01 02 3a fe 3a` | Verified: F1 (usage `3A`) |
| 4209 | 99.843750 | 8 | `01 00 00 01 02 45 fe 45` | Verified: F12 (usage `45`) |
| 12481 | 288.171875 | 20 | `01 00 00 0f 00 f1 0c fe 0c 2c fe f1 09 fe 2c fe 09 12 fe 12` | Observed only: accidental operator-reported text `I fo`. Includes `F1` around usage `0C`. Do not label the exact macro Verified |
| 12813 | 295.515625 | 8 | `01 00 00 01 02 4c fe 4c` | Verified: Delete Forward (usage `4C`) |
| 13207 | 304.765625 | 8 | `01 00 00 01 02 4f fe 4f` | Verified: Right Arrow (usage `4F`) |
| 13703 | 315.953125 | 8 | `01 00 00 01 02 58 fe 58` | Verified: Keypad Enter (usage `58`) |

No-write (no request-6): F13, PrintScreen, F14, ScrollLock, F15, Pause, F24, Application/Menu.

### `xkeys-extended-modifiers-9832c176-c560-4dbb-b423-93afdf28edb0.pcap`

Twelve request-6 writes, in capture order. Token names follow payload bytes, not host intent. Clean chords have exactly one usage-`04` tap (no repeat).

| Frame | Time (s) | wLength | Payload | Classification |
| ---: | ---: | ---: | --- | --- |
| 3389 | 84.156250 | 11 | `01 00 00 02 04 f1 04 fe 04 fe f1` | Verified: LShift+A (`F1`) |
| 3743 | 92.421875 | 11 | `01 00 00 02 04 f2 04 fe 04 fe f2` | Verified: LAlt+A (`F2`) |
| 4121 | 101.062500 | 13 | `01 00 00 09 ff fe f0 f0 04 fe 04 fe f0` | Observed; do not verify: intended LGUI+A, malformed/intercepted (`09 FF`, doubled `F0`) |
| 4615 | 112.328125 | 11 | `01 00 00 02 04 f7 04 fe 04 fe f7` | Observed / uncertain: intended RCtrl+A; payload is `F7` (host remap). Same bytes as later RGUI+A. Not RCtrl verification |
| 5169 | 125.093750 | 11 | `01 00 00 02 04 f5 04 fe 04 fe f5` | Verified: RShift+A (`F5`) |
| 6663 | 161.125000 | 11 | `01 00 00 02 04 f6 04 fe 04 fe f6` | Verified: RAlt+A (`F6`) |
| 9243 | 225.203125 | 11 | `01 00 00 02 04 f7 04 fe 04 fe f7` | Verified: RGUI+A (`F7`) |
| 9603 | 233.531250 | 14 | `01 00 00 03 06 f0 f1 04 fe 04 fe f1 fe f0` | Verified: Ctrl+Shift+A (`F0` `F1`) |
| 9935 | 241.000000 | 14 | `01 00 00 03 06 f0 f2 04 fe 04 fe f0 fe f2` | Verified: Ctrl+Alt+A (`F0` `F2`) |
| 10331 | 249.968750 | 14 | `01 00 00 03 06 f1 f2 04 fe 04 fe f1 fe f2` | Verified: Shift+Alt+A (`F1` `F2`) |
| 10757 | 259.609375 | 17 | `01 00 00 04 08 f0 f1 f2 04 fe 04 fe f2 fe f0 fe f1` | Verified: Ctrl+Shift+Alt+A (`F0` `F1` `F2`) |
| 11215 | 269.890625 | 20 | `01 00 00 05 0a f0 f1 f2 f3 04 fe 04 fe f3 fe f2 fe f0 fe f1` | Verified: Ctrl+Shift+Alt+GUI+A (`F0` `F1` `F2` `F3`). `F3` is confirmed in this multi-modifier row |

### `xkeys-extended-sequences-8b4e33be-a9f6-4bfa-a335-0237b2047aee.pcap`

Eight request-6 writes, in capture order. After a,b,c the operator attempted F13 then F24 (no-write). After clear, extra pedal taps produced no further request-6.

| Frame | Time (s) | wLength | Payload | Classification |
| ---: | ---: | ---: | --- | --- |
| 3897 | 95.921875 | 11 | `01 00 00 02 04 f4 04 fe 04 fe f4` | Verified: RCtrl+A (`F4`) |
| 4443 | 108.093750 | 8 | `03 00 00 01 02 04 fe 04` | Verified: Pedal C->a (selector `03`) |
| 4995 | 120.906250 | 14 | `01 00 00 09 00 04 fe 04 05 fe 05 06 fe 06` | Verified: a,b,c |
| 6783 | 161.734375 | 14 | `01 00 00 09 00 f1 04 fe 04 fe f1 05 fe 05` | Verified: Shift+A then b (`F1` then usage `05`) |
| 7283 | 173.062500 | 17 | `01 00 00 0c 00 f3 04 fe 04 fe f3 f1 05 fe 05 fe f1` | Observed: intended Ctrl-containing sequence; payload is `F3`/`F1` (GUI-sequence evidence, not a verified Ctrl label) |
| 7607 | 180.296875 | 16 | `01 00 00 07 04 f3 04 fe 04 fe f3 f3 fe 05 fe f3` | Observed: intended Ctrl-containing sequence; payload is `F3` (GUI-sequence evidence, not a verified Ctrl label) |
| 8183 | 192.875000 | 17 | `01 00 00 0c 00 f3 f1 04 fe 04 fe f1 fe f3 05 fe 05` | Observed: intended Ctrl-containing sequence; payload is `F3`/`F1` (GUI-sequence evidence, not a verified Ctrl label) |
| 8663 | 203.625000 | 5 | `01 00 00 00 00` | Observed / labelled: Escape x3 clear |

### `xkeys-mouse-advanced-7c2f9a11-4e60-4b8d-9c1a-restart2.pcap`

Nine request-6 writes in operator order. Device-wide erase produced no request 6; see request 8 below.

| Frame | Time (s) | wLength | Payload | Scenario |
| ---: | ---: | ---: | --- | --- |
| 8469 | 208.812500 | 9 | `01 20 00 04 00 01 00 00 00` | Verified: Pedal A left click (`Esc b 1`) |
| 8975 | 219.968750 | 9 | `02 20 00 04 00 02 00 00 00` | Verified: Pedal B right click (`Esc b 2`) |
| 9677 | 235.109375 | 9 | `03 20 00 04 00 04 00 00 00` | Verified: Pedal C middle click (`Esc b 3`) |
| 10439 | 252.265625 | 9 | `01 20 00 04 00 00 00 00 01` | Verified: Pedal A self-scroll up (`Esc s 1`) |
| 11145 | 267.531250 | 9 | `02 20 00 04 00 00 00 00 ff` | Verified: Pedal B self-scroll down (`Esc s -1`) |
| 12215 | 290.796875 | 11 | `03 00 00 03 03 04 fe 04 05 fe 05` | Observed: intended Pedal C `a`, delay, `b`. Body is `a` then `b`; no extra delay token |
| 12997 | 308.187500 | 10 | `01 00 00 06 ff fe f1 1b fe 1b` | Observed: intended Pedal A repeat-toggle `x`. Payload includes `F1` and usage `1B` (X) |
| 13983 | 329.203125 | 11 | `02 00 00 03 03 04 fe 04 05 fe 05` | Observed: intended Pedal B press `a` / release `b`. Same `a` then `b` body as frame 12215 |
| 14367 | 337.546875 | 8 | `03 00 00 01 02 f0 fe f0` | Verified: Pedal C Left Ctrl only |

Request 8 immediately after the last request-3 (frame 15051), operator-labelled device-wide erase (`Esc`, Backspace, `Esc` ×3):

| Frame | Time (s) | Envelope | Payload | Scenario |
| ---: | ---: | --- | --- | --- |
| 15053 | 352.671875 | `0x42` / `bRequest=8` / `wLength=1` | `08` | Observed: intended device-wide erase. Not a request-6 write |

Each of the nine request-6 writes is preceded by request 3 IN (`0xC2` / `wLength=7`). Complete-stage data for those IN transfers is empty in this file.

## Inspected and not stored

Inspected during recovery; **no** `0x42` / request-6 frames, so they are not successful programming evidence and are not in this repository:

| Filename | SHA-256 | Bytes |
| --- | --- | --- |
| `xkeys-programming-capture.pcap` | `78139228f01fe815ba2e80bbee7584f61846de26614170be401acfc933dd66e4` | 414 |
| `xkeys-programming.pcap` | `b4514ef1e3d611a0b6c81ba675f99e9b1fad113e87fad46b7a42d8159eae8d68` | 98398 |
| `xkeys-scheduled-8c2877b8-e98a-4f09-80b7-93aa513ff8ee.pcap` | `930dc66be405c0b1f576a40b9b6350bd49f09ecd09e8177c3b4a9747a4fba6d1` | 196772 |
