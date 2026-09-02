//! Play-mode live monitor.
//!
//! macOS and other non-Windows targets use hidapi boot-keyboard reports. Windows
//! uses a separate device-filtered Raw Input backend so Play PID `05F3:030C`
//! can stay on the HID driver. Raw Input `RAWKEYBOARD` fields are not 8-byte
//! HID reports.

use anyhow::Result;
use serde::Serialize;

#[cfg(not(windows))]
use anyhow::{anyhow, Context};
#[cfg(not(windows))]
use std::time::Duration;

use crate::cli::SavantElite;
#[cfg(not(windows))]
use crate::protocol::usb_hid;

#[cfg(not(windows))]
use crate::protocol::{KINESIS_VID, SAVANT_ELITE_PID};
#[cfg(not(windows))]
use crate::transport::new_hid_api;
#[cfg(not(windows))]
use hidapi::HidDevice;

/// Case-folded Play-mode identity token used in Win32 device interface paths.
pub const PLAY_RAW_DEVICE_TOKEN: &str = "VID_05F3&PID_030C";

/// `RI_KEY_BREAK` from the Win32 `RAWKEYBOARD` flags (key-up).
pub const RAW_KEY_BREAK: u16 = 1;

/// Win32 virtual-key codes used for Play-mode verification decoding.
/// See: https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes
pub const VK_CONTROL: u16 = 0x11;
pub const VK_LCONTROL: u16 = 0xA2;
pub const VK_RCONTROL: u16 = 0xA3;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_LSHIFT: u16 = 0xA0;
pub const VK_RSHIFT: u16 = 0xA1;
pub const VK_MENU: u16 = 0x12;
pub const VK_LMENU: u16 = 0xA4;
pub const VK_RMENU: u16 = 0xA5;
pub const VK_LWIN: u16 = 0x5B;
pub const VK_RWIN: u16 = 0x5C;
pub const VK_A: u16 = 0x41;
pub const VK_Z: u16 = 0x5A;
pub const VK_PAUSE: u16 = 0x13;
pub const VK_PRIOR: u16 = 0x21;
pub const VK_NEXT: u16 = 0x22;
pub const VK_END: u16 = 0x23;
pub const VK_HOME: u16 = 0x24;
pub const VK_LEFT: u16 = 0x25;
pub const VK_UP: u16 = 0x26;
pub const VK_RIGHT: u16 = 0x27;
pub const VK_DOWN: u16 = 0x28;
pub const VK_SNAPSHOT: u16 = 0x2C;
pub const VK_INSERT: u16 = 0x2D;
pub const VK_DELETE: u16 = 0x2E;
pub const VK_APPS: u16 = 0x5D;
pub const VK_NUMPAD0: u16 = 0x60;
pub const VK_NUMPAD9: u16 = 0x69;
pub const VK_MULTIPLY: u16 = 0x6A;
pub const VK_ADD: u16 = 0x6B;
pub const VK_SUBTRACT: u16 = 0x6D;
pub const VK_DECIMAL: u16 = 0x6E;
pub const VK_DIVIDE: u16 = 0x6F;
pub const VK_F1: u16 = 0x70;
pub const VK_F24: u16 = 0x87;
pub const VK_NUMLOCK: u16 = 0x90;
pub const VK_SCROLL: u16 = 0x91;

/// Press versus release from `RAWKEYBOARD.Flags`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPhase {
    Press,
    Release,
}

/// Human-readable decode of one virtual-key code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedKey {
    pub name: String,
    pub is_modifier: bool,
}

/// Normalize a raw-input device interface path for identity matching.
#[must_use]
pub fn normalize_raw_device_path(path: &str) -> String {
    path.replace('/', "\\").to_ascii_uppercase()
}

/// Accept only Play PID `05F3:030C`. Host keyboards must not match.
#[must_use]
pub fn is_play_raw_device_path(path: &str) -> bool {
    normalize_raw_device_path(path).contains(PLAY_RAW_DEVICE_TOKEN)
}

/// Classify a `RAWKEYBOARD.Flags` value as press or release.
#[must_use]
pub fn classify_raw_key_phase(flags: u16) -> KeyPhase {
    if flags & RAW_KEY_BREAK == 0 {
        KeyPhase::Press
    } else {
        KeyPhase::Release
    }
}

/// Decode a Win32 virtual-key code for Play-mode verification.
///
/// Letters are lowercase (`VK_A` → `a`) so Pedal A→a is readable. Left/right
/// Control variants all become `Ctrl`.
#[must_use]
pub fn decode_virtual_key(vkey: u16) -> Option<DecodedKey> {
    match vkey {
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL => Some(DecodedKey {
            name: "Ctrl".to_string(),
            is_modifier: true,
        }),
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT => Some(DecodedKey {
            name: "Shift".to_string(),
            is_modifier: true,
        }),
        VK_MENU | VK_LMENU | VK_RMENU => Some(DecodedKey {
            name: "Alt".to_string(),
            is_modifier: true,
        }),
        VK_LWIN | VK_RWIN => Some(DecodedKey {
            name: "Win".to_string(),
            is_modifier: true,
        }),
        VK_A..=VK_Z => {
            let letter = b'a' + u8::try_from(vkey - VK_A).ok()?;
            Some(DecodedKey {
                name: char::from(letter).to_string(),
                is_modifier: false,
            })
        }
        0x30..=0x39 => {
            let digit = b'0' + u8::try_from(vkey - 0x30).ok()?;
            Some(DecodedKey {
                name: char::from(digit).to_string(),
                is_modifier: false,
            })
        }
        0x08 => Some(DecodedKey {
            name: "Backspace".to_string(),
            is_modifier: false,
        }),
        0x09 => Some(DecodedKey {
            name: "Tab".to_string(),
            is_modifier: false,
        }),
        0x0D => Some(DecodedKey {
            name: "Enter".to_string(),
            is_modifier: false,
        }),
        0x1B => Some(DecodedKey {
            name: "Escape".to_string(),
            is_modifier: false,
        }),
        0x20 => Some(DecodedKey {
            name: "Space".to_string(),
            is_modifier: false,
        }),
        VK_PAUSE => Some(DecodedKey {
            name: "Pause".to_string(),
            is_modifier: false,
        }),
        VK_PRIOR => Some(DecodedKey {
            name: "PageUp".to_string(),
            is_modifier: false,
        }),
        VK_NEXT => Some(DecodedKey {
            name: "PageDown".to_string(),
            is_modifier: false,
        }),
        VK_END => Some(DecodedKey {
            name: "End".to_string(),
            is_modifier: false,
        }),
        VK_HOME => Some(DecodedKey {
            name: "Home".to_string(),
            is_modifier: false,
        }),
        VK_LEFT => Some(DecodedKey {
            name: "Left".to_string(),
            is_modifier: false,
        }),
        VK_UP => Some(DecodedKey {
            name: "Up".to_string(),
            is_modifier: false,
        }),
        VK_RIGHT => Some(DecodedKey {
            name: "Right".to_string(),
            is_modifier: false,
        }),
        VK_DOWN => Some(DecodedKey {
            name: "Down".to_string(),
            is_modifier: false,
        }),
        VK_SNAPSHOT => Some(DecodedKey {
            name: "PrintScreen".to_string(),
            is_modifier: false,
        }),
        VK_INSERT => Some(DecodedKey {
            name: "Insert".to_string(),
            is_modifier: false,
        }),
        VK_DELETE => Some(DecodedKey {
            name: "Delete".to_string(),
            is_modifier: false,
        }),
        VK_APPS => Some(DecodedKey {
            name: "Application".to_string(),
            is_modifier: false,
        }),
        VK_NUMPAD0..=VK_NUMPAD9 => {
            let digit = b'0' + u8::try_from(vkey - VK_NUMPAD0).ok()?;
            Some(DecodedKey {
                name: format!("Keypad{}", char::from(digit)),
                is_modifier: false,
            })
        }
        VK_MULTIPLY => Some(DecodedKey {
            name: "KeypadMultiply".to_string(),
            is_modifier: false,
        }),
        VK_ADD => Some(DecodedKey {
            name: "KeypadAdd".to_string(),
            is_modifier: false,
        }),
        VK_SUBTRACT => Some(DecodedKey {
            name: "KeypadSubtract".to_string(),
            is_modifier: false,
        }),
        VK_DECIMAL => Some(DecodedKey {
            name: "KeypadDecimal".to_string(),
            is_modifier: false,
        }),
        VK_DIVIDE => Some(DecodedKey {
            name: "KeypadDivide".to_string(),
            is_modifier: false,
        }),
        VK_F1..=VK_F24 => {
            let number = vkey - VK_F1 + 1;
            Some(DecodedKey {
                name: format!("F{number}"),
                is_modifier: false,
            })
        }
        VK_NUMLOCK => Some(DecodedKey {
            name: "NumLock".to_string(),
            is_modifier: false,
        }),
        VK_SCROLL => Some(DecodedKey {
            name: "ScrollLock".to_string(),
            is_modifier: false,
        }),
        _ => Some(DecodedKey {
            name: format!("VK_{vkey:02X}"),
            is_modifier: false,
        }),
    }
}

/// Update the Play-device modifier set from one decoded event.
pub fn apply_modifier_state(held: &mut Vec<String>, key: &str, is_modifier: bool, phase: KeyPhase) {
    if !is_modifier {
        return;
    }
    match phase {
        KeyPhase::Press => {
            if !held.iter().any(|item| item == key) {
                held.push(key.to_string());
            }
        }
        KeyPhase::Release => held.retain(|item| item != key),
    }
}

/// Human-readable combo such as `a` or `Ctrl+a`.
#[must_use]
pub fn format_key_display(held: &[String], key: &str, is_modifier: bool) -> String {
    if is_modifier || held.is_empty() {
        key.to_string()
    } else {
        format!("{}+{key}", held.join("+"))
    }
}

/// Win32 `RIM_TYPEKEYBOARD`.
pub const RIM_TYPE_KEYBOARD: u32 = 1;

/// Documented `RAWKEYBOARD` size (same on x86 and x64).
pub const RAWKEYBOARD_BYTE_SIZE: usize = 16;

/// Typical x64 `sizeof(RAWINPUT)` when the union is sized to `RAWMOUSE` (header 24 + 24).
/// Keyboard events often copy only header + `RAWKEYBOARD` (40) and must still parse.
pub const RAWINPUT_X64_UNION_MAX_SIZE: usize = 48;

/// Why a `GetRawInputData` buffer could not be decoded as a keyboard event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawInputParseError {
    SizeQueryFailed,
    DataCopyFailed,
    ShortHeader,
    ShortKeyboard,
    NonKeyboard { dw_type: u32 },
}

impl RawInputParseError {
    #[must_use]
    pub fn reason_name(self) -> &'static str {
        match self {
            Self::SizeQueryFailed => "size_query_failed",
            Self::DataCopyFailed => "data_copy_failed",
            Self::ShortHeader => "short_header",
            Self::ShortKeyboard => "short_keyboard",
            Self::NonKeyboard { .. } => "non_keyboard",
        }
    }
}

/// `RAWINPUTHEADER` fields decoded from a copied byte buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedRawInputHeader {
    pub dw_type: u32,
    pub dw_size: u32,
    pub h_device: u64,
    pub w_param: u64,
}

/// `RAWKEYBOARD` fields used for Play-mode verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedRawKeyboard {
    pub make_code: u16,
    pub flags: u16,
    pub vkey: u16,
    pub message: u32,
}

/// Byte size of `RAWINPUTHEADER` for a given pointer width (4 = x86, 8 = x64).
#[must_use]
pub fn rawinput_header_byte_size(pointer_width: usize) -> usize {
    8 + 2 * pointer_width
}

/// Minimum copied bytes for a keyboard `WM_INPUT` (header + `RAWKEYBOARD`).
#[must_use]
pub fn raw_keyboard_event_byte_size(pointer_width: usize) -> usize {
    rawinput_header_byte_size(pointer_width) + RAWKEYBOARD_BYTE_SIZE
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    bytes
        .get(offset..offset.checked_add(2)?)
        .and_then(|slice| slice.try_into().ok())
        .map(u16::from_le_bytes)
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset.checked_add(4)?)
        .and_then(|slice| slice.try_into().ok())
        .map(u32::from_le_bytes)
}

fn read_ptr_le(bytes: &[u8], offset: usize, pointer_width: usize) -> Option<u64> {
    match pointer_width {
        4 => read_u32_le(bytes, offset).map(u64::from),
        8 => bytes
            .get(offset..offset.checked_add(8)?)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes),
        _ => None,
    }
}

/// Parse `RAWINPUTHEADER` from the copied `GetRawInputData` bytes.
pub fn parse_rawinput_header(
    bytes: &[u8],
    pointer_width: usize,
) -> Result<ParsedRawInputHeader, RawInputParseError> {
    let header_len = rawinput_header_byte_size(pointer_width);
    if bytes.len() < header_len {
        return Err(RawInputParseError::ShortHeader);
    }
    let Some(dw_type) = read_u32_le(bytes, 0) else {
        return Err(RawInputParseError::ShortHeader);
    };
    let Some(dw_size) = read_u32_le(bytes, 4) else {
        return Err(RawInputParseError::ShortHeader);
    };
    let Some(h_device) = read_ptr_le(bytes, 8, pointer_width) else {
        return Err(RawInputParseError::ShortHeader);
    };
    let Some(w_param) = read_ptr_le(bytes, 8 + pointer_width, pointer_width) else {
        return Err(RawInputParseError::ShortHeader);
    };
    Ok(ParsedRawInputHeader {
        dw_type,
        dw_size,
        h_device,
        w_param,
    })
}

/// Parse a keyboard `WM_INPUT` using the copied byte count, not `sizeof(RAWINPUT)`.
pub fn parse_raw_keyboard_event(
    copied: &[u8],
    pointer_width: usize,
) -> Result<(ParsedRawInputHeader, ParsedRawKeyboard), RawInputParseError> {
    let header = parse_rawinput_header(copied, pointer_width)?;
    if header.dw_type != RIM_TYPE_KEYBOARD {
        return Err(RawInputParseError::NonKeyboard {
            dw_type: header.dw_type,
        });
    }
    let kb_off = rawinput_header_byte_size(pointer_width);
    if copied.len() < kb_off + RAWKEYBOARD_BYTE_SIZE {
        return Err(RawInputParseError::ShortKeyboard);
    }
    let Some(make_code) = read_u16_le(copied, kb_off) else {
        return Err(RawInputParseError::ShortKeyboard);
    };
    let Some(flags) = read_u16_le(copied, kb_off + 2) else {
        return Err(RawInputParseError::ShortKeyboard);
    };
    let Some(vkey) = read_u16_le(copied, kb_off + 6) else {
        return Err(RawInputParseError::ShortKeyboard);
    };
    let Some(message) = read_u32_le(copied, kb_off + 8) else {
        return Err(RawInputParseError::ShortKeyboard);
    };
    Ok((
        header,
        ParsedRawKeyboard {
            make_code,
            flags,
            vkey,
            message,
        },
    ))
}

/// Normalize a HID input report to an 8-byte boot keyboard report.
///
/// Some HID backends include a leading report-id byte (often 0) and/or pad
/// input reports out to a larger endpoint size. The Savant Elite keyboard
/// interface is a standard 8-byte boot keyboard report, so we normalize to
/// the first 8 bytes of the actual report.
pub fn normalize_boot_keyboard_report(data: &[u8]) -> Option<[u8; 8]> {
    if data.len() < 8 {
        return None;
    }

    let looks_prefixed = data.len() >= 9
        && data[0] == 0
        && data[2] == 0
        && (data[1] != 0 || data[3..9].iter().any(|&b| b != 0));
    let offset = if looks_prefixed { 1 } else { 0 };
    if data.len() < offset + 8 {
        return None;
    }

    let mut report = [0u8; 8];
    report.copy_from_slice(&data[offset..offset + 8]);
    Some(report)
}

#[derive(Serialize)]
struct JsonMonitorEvent {
    backend: &'static str,
    event: &'static str,
    display: String,
    key: String,
    modifiers: Vec<String>,
    source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    hid_report_hex: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rawkeyboard: Option<JsonRawKeyboard>,
}

#[derive(Serialize)]
struct JsonRawKeyboard {
    make_code: u16,
    vkey: u16,
    flags: u16,
    message: u32,
    note: &'static str,
}

impl SavantElite {
    pub(crate) fn monitor(&self, duration_secs: u64) -> Result<()> {
        #[cfg(windows)]
        {
            self.monitor_windows_raw_input(duration_secs)
        }
        #[cfg(not(windows))]
        {
            self.monitor_hidapi(duration_secs)
        }
    }

    fn print_monitor_banner(&self, backend_line: &str, duration_secs: u64) {
        if self.json_output {
            return;
        }

        self.print_banner();
        self.console.print(
            "[bold #9b59b6]┌─────────────────────────────────────────────────────────────────┐[/]",
        );
        self.console.print(
            "[bold #9b59b6]│[/]  [bold #f39c12]👁[/]  [bold white]LIVE MONITOR MODE[/]                                          [bold #9b59b6]│[/]",
        );
        self.console.print(
            "[bold #9b59b6]└─────────────────────────────────────────────────────────────────┘[/]",
        );
        self.console.print("");
        self.console
            .print(&format!("  [#95a5a6]{}[/]", backend_line));
        self.console
            .print("  [#95a5a6]Press a pedal to see what key it sends.[/]");
        self.console
            .print("  [#95a5a6]Press[/] [bold #e74c3c]Ctrl+C[/] [#95a5a6]to stop.[/]");
        if duration_secs > 0 {
            self.console.print(&format!(
                "  [#95a5a6]Auto-stop in[/] [bold #f39c12]{}[/] [#95a5a6]seconds.[/]",
                duration_secs
            ));
        }
        self.console.print("");
        self.console.print(
            "[#3498db]─────────────────────────────────────────────────────────────────────[/]",
        );
    }

    fn print_monitor_complete(&self) {
        if self.json_output {
            return;
        }
        self.console.print("");
        self.console.print(
            "[#3498db]─────────────────────────────────────────────────────────────────────[/]",
        );
        self.console
            .print("[bold #2ecc71]✓[/] [#95a5a6]Monitoring complete.[/]");
        self.console.print("");
    }

    #[cfg(not(windows))]
    fn emit_hid_event(&self, combo: &str, phase: KeyPhase, report: &[u8; 8]) {
        if self.json_output {
            let event = JsonMonitorEvent {
                backend: "hidapi",
                event: match phase {
                    KeyPhase::Press => "press",
                    KeyPhase::Release => "release",
                },
                display: combo.to_string(),
                key: combo.to_string(),
                modifiers: usb_hid::modifier_names(report[0])
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                source: "hid-boot-report",
                hid_report_hex: Some(hex::encode(report)),
                rawkeyboard: None,
            };
            println!(
                "{}",
                serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string())
            );
            return;
        }

        self.verbose(&format!("hid_report_hex {}", hex::encode(report)));
        match phase {
            KeyPhase::Press => self.console.print(&format!(
                "  [bold #2ecc71]▶[/] [bold #f1c40f]PRESS[/]   [bold white]{}[/]",
                combo
            )),
            KeyPhase::Release => self.console.print(&format!(
                "  [dim #e74c3c]◀[/] [dim #95a5a6]RELEASE[/] [dim white]{}[/]",
                combo
            )),
        }
    }

    #[cfg(windows)]
    fn emit_rawkeyboard_event(
        &self,
        display: &str,
        key: &str,
        modifiers: &[String],
        phase: KeyPhase,
        raw: JsonRawKeyboard,
    ) {
        self.verbose(&format!(
            "RAWKEYBOARD make=0x{:04X} vkey=0x{:04X} flags=0x{:04X} (not an 8-byte HID report)",
            raw.make_code, raw.vkey, raw.flags
        ));

        if self.json_output {
            let event = JsonMonitorEvent {
                backend: "windows-raw-input",
                event: match phase {
                    KeyPhase::Press => "press",
                    KeyPhase::Release => "release",
                },
                display: display.to_string(),
                key: key.to_string(),
                modifiers: modifiers.to_vec(),
                source: "rawkeyboard",
                hid_report_hex: None,
                rawkeyboard: Some(raw),
            };
            println!(
                "{}",
                serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string())
            );
            return;
        }

        match phase {
            KeyPhase::Press => self.console.print(&format!(
                "  [bold #2ecc71]▶[/] [bold #f1c40f]PRESS[/]   [bold white]{display}[/]"
            )),
            KeyPhase::Release => self.console.print(&format!(
                "  [dim #e74c3c]◀[/] [dim #95a5a6]RELEASE[/] [dim white]{display}[/]"
            )),
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn open_keyboard_interface(&self) -> Result<HidDevice> {
        self.verbose("Initializing HID API for keyboard interface...");
        let api = new_hid_api().context("Failed to initialize HID API")?;

        // Find the keyboard interface (usage page 1, usage 6)
        self.verbose("Searching for keyboard interface (usage_page=0x01, usage=0x06)...");
        for device in api.device_list() {
            if device.vendor_id() == KINESIS_VID
                && device.product_id() == SAVANT_ELITE_PID
                && device.usage_page() == 0x01
                && device.usage() == 0x06
            {
                self.verbose(&format!(
                    "Found keyboard interface at path: {}",
                    device.path().to_string_lossy()
                ));
                match device.open_device(&api) {
                    Ok(dev) => {
                        self.verbose("Keyboard interface opened successfully");
                        return Ok(dev);
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.contains("privilege violation") || msg.contains("0xE00002C1") {
                            return Err(anyhow!(e).context(
                                "Failed to open device (macOS Input Monitoring permission is required; enable it in System Settings → Privacy & Security → Input Monitoring, then re-run)",
                            ));
                        }
                        return Err(anyhow!(e).context("Failed to open device"));
                    }
                }
            }
        }

        Err(anyhow!("Savant Elite keyboard interface not found"))
    }

    #[cfg(not(windows))]
    fn monitor_hidapi(&self, duration_secs: u64) -> Result<()> {
        let device = self.open_keyboard_interface()?;
        self.print_monitor_banner("Watching pedal taps.", duration_secs);

        self.verbose("Setting non-blocking mode on HID device");
        device.set_blocking_mode(false)?;

        let mut buf = [0u8; 64];
        let mut last_report = [0u8; 8];
        let start = std::time::Instant::now();
        self.verbose("Starting hidapi monitor loop...");

        loop {
            if duration_secs > 0 && start.elapsed().as_secs() >= duration_secs {
                self.print_monitor_complete();
                break;
            }

            match device.read_timeout(&mut buf, 100) {
                Ok(len) if len > 0 => {
                    self.verbose_hex("Raw HID read", &buf[..len]);
                    let Some(report) = normalize_boot_keyboard_report(&buf[..len]) else {
                        self.verbose("  -> Could not normalize to boot keyboard report");
                        continue;
                    };

                    if report != last_report {
                        last_report = report;
                        self.verbose_hex("Normalized report", &report);

                        let modifiers = report[0];
                        let keys: Vec<u8> =
                            report[2..8].iter().filter(|&&k| k != 0).copied().collect();

                        if modifiers != 0 || !keys.is_empty() {
                            let mod_names = usb_hid::modifier_names(modifiers);
                            let key_names: Vec<&str> =
                                keys.iter().map(|&k| usb_hid::key_name(k)).collect();

                            let combo = if !mod_names.is_empty() && !key_names.is_empty() {
                                format!("{}+{}", mod_names.join("+"), key_names.join("+"))
                            } else if !mod_names.is_empty() {
                                mod_names.join("+")
                            } else {
                                key_names.join("+")
                            };

                            self.emit_hid_event(&combo, KeyPhase::Press, &report);
                        } else {
                            self.emit_hid_event("", KeyPhase::Release, &report);
                        }
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    if e.to_string().contains("timeout") {
                        continue;
                    }
                    return Err(anyhow!("Read error: {}", e));
                }
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        Ok(())
    }

    #[cfg(windows)]
    fn monitor_windows_raw_input(&self, duration_secs: u64) -> Result<()> {
        raw_input::run(self, duration_secs)
    }
}

#[cfg(windows)]
mod raw_input {
    use super::{
        apply_modifier_state, classify_raw_key_phase, decode_virtual_key, format_key_display,
        is_play_raw_device_path, SavantElite,
    };
    use anyhow::{anyhow, Context, Result};
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{
        GetLastError, FALSE, HANDLE, HWND, LPARAM, LRESULT, TRUE, WPARAM,
    };
    use windows_sys::Win32::System::Console::{
        SetConsoleCtrlHandler, CTRL_CLOSE_EVENT, CTRL_C_EVENT,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::{
        GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList,
        GetRegisteredRawInputDevices, RegisterRawInputDevices, HRAWINPUT, RAWINPUTDEVICE,
        RAWINPUTDEVICELIST, RAWINPUTHEADER, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDEV_REMOVE,
        RIDI_DEVICENAME, RID_INPUT, RIM_TYPEKEYBOARD,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetWindowLongPtrW,
        PeekMessageW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW, ShowWindow,
        TranslateMessage, UnregisterClassW, GIDC_ARRIVAL, GIDC_REMOVAL, GWLP_USERDATA, MSG,
        PM_REMOVE, SW_HIDE, WM_CLOSE, WM_DESTROY, WM_INPUT, WM_INPUT_DEVICE_CHANGE, WM_QUIT,
        WNDCLASSEXW, WS_POPUP,
    };

    const CLASS_NAME: &str = "SavantElite.PlayRawInput.v1";
    const KEYBOARD_USAGE_PAGE: u16 = 0x01;
    const KEYBOARD_USAGE: u16 = 0x06;
    const KEYBOARD_OVERRUN_MAKE_CODE: u16 = 0xFF;
    static STOP: AtomicBool = AtomicBool::new(false);

    struct RawInputSession<'a> {
        app: &'a SavantElite,
        path_cache: HashMap<usize, Option<String>>,
        held_modifiers: Vec<String>,
        wm_input_count: u64,
        accepted_count: u64,
        filtered_count: u64,
        size_query_fail: u64,
        data_copy_fail: u64,
        short_header: u64,
        short_keyboard: u64,
        non_keyboard: u64,
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn last_error() -> u32 {
        // SAFETY: GetLastError reads the calling thread's last-error code.
        unsafe { GetLastError() }
    }

    unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> i32 {
        match ctrl_type {
            CTRL_C_EVENT | CTRL_CLOSE_EVENT => {
                STOP.store(true, Ordering::SeqCst);
                TRUE
            }
            _ => FALSE,
        }
    }

    fn device_path(handle: HANDLE) -> Option<String> {
        if handle.is_null() {
            return None;
        }
        let mut chars = 0u32;
        // SAFETY: first call with a null buffer asks for the RIDI_DEVICENAME character count.
        let probe = unsafe {
            GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, std::ptr::null_mut(), &mut chars)
        };
        if chars == 0 || probe == u32::MAX {
            return None;
        }
        let mut buf = vec![0u16; chars as usize];
        // SAFETY: `buf` is a writable UTF-16 buffer of `chars` code units.
        let copied = unsafe {
            GetRawInputDeviceInfoW(
                handle,
                RIDI_DEVICENAME,
                buf.as_mut_ptr().cast::<c_void>(),
                &mut chars,
            )
        };
        if copied == u32::MAX {
            return None;
        }
        let end = buf.iter().position(|&unit| unit == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    fn list_play_device_paths() -> Result<Vec<String>> {
        let mut count = 0u32;
        let header = u32::try_from(std::mem::size_of::<RAWINPUTDEVICELIST>())
            .context("RAWINPUTDEVICELIST size does not fit in u32")?;
        // SAFETY: null list pointer returns the attached raw-input device count.
        let probe = unsafe { GetRawInputDeviceList(std::ptr::null_mut(), &mut count, header) };
        if probe == u32::MAX {
            return Err(anyhow!(
                "GetRawInputDeviceList failed (GetLastError={})",
                last_error()
            ));
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut devices = vec![RAWINPUTDEVICELIST::default(); count as usize];
        // SAFETY: `devices` is an array of `count` RAWINPUTDEVICELIST structures.
        let filled = unsafe { GetRawInputDeviceList(devices.as_mut_ptr(), &mut count, header) };
        if filled == u32::MAX {
            return Err(anyhow!(
                "GetRawInputDeviceList failed (GetLastError={})",
                last_error()
            ));
        }
        let mut paths = Vec::new();
        for device in devices.into_iter().take(filled as usize) {
            if device.dwType != RIM_TYPEKEYBOARD {
                continue;
            }
            if let Some(path) = device_path(device.hDevice) {
                if is_play_raw_device_path(&path) {
                    paths.push(path);
                }
            }
        }
        Ok(paths)
    }

    fn no_play_device_error() -> anyhow::Error {
        anyhow!(
            "No pedal found in Play mode. Connect the pedal via USB, flip the switch to Play, then unplug and replug. \
Never bind Play PID 05F3:030C to WinUSB; leave it on the HID driver."
        )
    }

    impl RawInputSession<'_> {
        fn note_parse_error(&mut self, error: super::RawInputParseError, copied: usize) {
            match error {
                super::RawInputParseError::SizeQueryFailed => {
                    self.size_query_fail = self.size_query_fail.saturating_add(1);
                }
                super::RawInputParseError::DataCopyFailed => {
                    self.data_copy_fail = self.data_copy_fail.saturating_add(1);
                }
                super::RawInputParseError::ShortHeader => {
                    self.short_header = self.short_header.saturating_add(1);
                }
                super::RawInputParseError::ShortKeyboard => {
                    self.short_keyboard = self.short_keyboard.saturating_add(1);
                }
                super::RawInputParseError::NonKeyboard { .. } => {
                    self.non_keyboard = self.non_keyboard.saturating_add(1);
                }
            }
            let extra = match error {
                super::RawInputParseError::NonKeyboard { dw_type } => {
                    format!(" type={dw_type}")
                }
                super::RawInputParseError::ShortHeader => format!(
                    " copied={copied} need={}",
                    super::rawinput_header_byte_size(std::mem::size_of::<usize>())
                ),
                super::RawInputParseError::ShortKeyboard => format!(
                    " copied={copied} need={}",
                    super::raw_keyboard_event_byte_size(std::mem::size_of::<usize>())
                ),
                _ => format!(" copied={copied}"),
            };
            self.app.verbose(&format!(
                "WM_INPUT parse={}{extra} (no key contents)",
                error.reason_name()
            ));
        }

        fn accept_device(&mut self, handle: HANDLE) -> bool {
            let key = handle as usize;
            if let Some(cached) = self.path_cache.get(&key) {
                return cached
                    .as_ref()
                    .is_some_and(|path| is_play_raw_device_path(path));
            }
            let path = device_path(handle);
            let accepted = path.as_deref().is_some_and(is_play_raw_device_path);
            if let Some(ref path) = path {
                self.app.verbose(&format!(
                    "Raw Input device path: {path} accepted={accepted}"
                ));
            }
            self.path_cache.insert(key, path);
            accepted
        }

        fn handle_wm_input(&mut self, lparam: LPARAM) {
            let hrawinput = lparam as HRAWINPUT;
            let header_size = u32::try_from(std::mem::size_of::<RAWINPUTHEADER>()).unwrap_or(0);
            let mut size = 0u32;
            // SAFETY: null pdata; success is 0 and pcbSize holds the required byte count.
            let queried = unsafe {
                GetRawInputData(
                    hrawinput,
                    RID_INPUT,
                    std::ptr::null_mut(),
                    &mut size,
                    header_size,
                )
            };
            if queried == u32::MAX || size == 0 {
                self.note_parse_error(super::RawInputParseError::SizeQueryFailed, 0);
                return;
            }
            let mut buf = vec![0u8; size as usize];
            // SAFETY: `buf` is the size reported by the query; return value is bytes copied.
            let copied = unsafe {
                GetRawInputData(
                    hrawinput,
                    RID_INPUT,
                    buf.as_mut_ptr().cast::<c_void>(),
                    &mut size,
                    header_size,
                )
            };
            if copied == u32::MAX || copied == 0 {
                self.note_parse_error(super::RawInputParseError::DataCopyFailed, 0);
                return;
            }
            let copied = copied as usize;
            let used = copied.min(buf.len());
            let (header, keyboard) =
                match super::parse_raw_keyboard_event(&buf[..used], std::mem::size_of::<usize>()) {
                    Ok(parsed) => parsed,
                    Err(error) => {
                        self.note_parse_error(error, used);
                        return;
                    }
                };
            let device = header.h_device as usize as HANDLE;
            let accepted = self.accept_device(device);
            if !accepted {
                self.filtered_count = self.filtered_count.saturating_add(1);
                return;
            }
            self.accepted_count = self.accepted_count.saturating_add(1);
            if keyboard.make_code == KEYBOARD_OVERRUN_MAKE_CODE || keyboard.vkey >= 0xFF {
                return;
            }
            let Some(decoded) = decode_virtual_key(keyboard.vkey) else {
                return;
            };
            let phase = classify_raw_key_phase(keyboard.flags);
            apply_modifier_state(
                &mut self.held_modifiers,
                &decoded.name,
                decoded.is_modifier,
                phase,
            );
            let display =
                format_key_display(&self.held_modifiers, &decoded.name, decoded.is_modifier);
            self.app.emit_rawkeyboard_event(
                &display,
                &decoded.name,
                &self.held_modifiers,
                phase,
                super::JsonRawKeyboard {
                    make_code: keyboard.make_code,
                    vkey: keyboard.vkey,
                    flags: keyboard.flags,
                    message: keyboard.message,
                    note: "Win32 RAWKEYBOARD diagnostics; not an 8-byte HID report",
                },
            );
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_INPUT => {
                let session = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RawInputSession<'_>;
                if !session.is_null() {
                    // SAFETY: session pointer is set on the same thread before registration.
                    (*session).wm_input_count = (*session).wm_input_count.saturating_add(1);
                    (*session).handle_wm_input(lparam);
                }
                // WM_INPUT requires DefWindowProc so the system can perform cleanup
                // (documented for RIM_INPUT; also used after RIM_INPUTSINK extraction).
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_INPUT_DEVICE_CHANGE => {
                let session = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RawInputSession<'_>;
                if !session.is_null() {
                    // SAFETY: same-thread userdata; lParam is the raw-input device HANDLE.
                    let play = (*session).accept_device(lparam as HANDLE);
                    let kind = match wparam as u32 {
                        GIDC_ARRIVAL => "arrival",
                        GIDC_REMOVAL => "removal",
                        _ => "change",
                    };
                    (*session).app.verbose(&format!(
                        "WM_INPUT_DEVICE_CHANGE {kind} play={play} (no key contents)"
                    ));
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    fn register_sink(hwnd: HWND) -> Result<()> {
        let devices = [RAWINPUTDEVICE {
            usUsagePage: KEYBOARD_USAGE_PAGE,
            usUsage: KEYBOARD_USAGE,
            dwFlags: requested_raw_input_flags(),
            hwndTarget: hwnd,
        }];
        let size = u32::try_from(std::mem::size_of::<RAWINPUTDEVICE>())
            .context("RAWINPUTDEVICE size does not fit in u32")?;
        // SAFETY: one keyboard usage-page registration targeted at our message window.
        let ok = unsafe { RegisterRawInputDevices(devices.as_ptr(), 1, size) };
        if ok == FALSE {
            return Err(anyhow!(
                "RegisterRawInputDevices failed (GetLastError={}). \
Try an interactive desktop session. Raw Input does not require binding Play PID 05F3:030C to WinUSB.",
                last_error()
            ));
        }
        Ok(())
    }

    fn requested_raw_input_flags() -> u32 {
        RIDEV_INPUTSINK | RIDEV_DEVNOTIFY
    }

    fn log_registered_devices(app: &SavantElite, expected_hwnd: HWND) {
        let Ok(size) = u32::try_from(std::mem::size_of::<RAWINPUTDEVICE>()) else {
            return;
        };
        let mut count = 0u32;
        // SAFETY: null buffer returns the registered-device count via ERROR_INSUFFICIENT_BUFFER.
        let probe = unsafe { GetRegisteredRawInputDevices(std::ptr::null_mut(), &mut count, size) };
        if probe == u32::MAX && count == 0 {
            app.verbose(&format!(
                "GetRegisteredRawInputDevices failed (GetLastError={})",
                last_error()
            ));
            return;
        }
        if count == 0 {
            app.verbose(
                "GetRegisteredRawInputDevices: count=0 (RegisterRawInputDevices returned success but nothing is registered)",
            );
            return;
        }
        let mut devices = vec![RAWINPUTDEVICE::default(); count as usize];
        // SAFETY: `devices` holds `count` RAWINPUTDEVICE structures.
        let filled =
            unsafe { GetRegisteredRawInputDevices(devices.as_mut_ptr(), &mut count, size) };
        if filled == u32::MAX {
            app.verbose(&format!(
                "GetRegisteredRawInputDevices fill failed (GetLastError={})",
                last_error()
            ));
            return;
        }
        app.verbose(&format!(
            "GetRegisteredRawInputDevices: count={filled} expected hwnd={expected_hwnd:p}"
        ));
        for (index, device) in devices.into_iter().take(filled as usize).enumerate() {
            let hwnd_match = device.hwndTarget == expected_hwnd;
            let sink = device.dwFlags & RIDEV_INPUTSINK != 0;
            let notify = device.dwFlags & RIDEV_DEVNOTIFY != 0;
            app.verbose(&format!(
                "  registered[{index}] usage_page=0x{:04X} usage=0x{:04X} flags=0x{:08X} inputsink={sink} devnotify={notify} hwnd_match={hwnd_match}",
                device.usUsagePage, device.usUsage, device.dwFlags
            ));
        }
    }

    fn unregister_sink() {
        let devices = [RAWINPUTDEVICE {
            usUsagePage: KEYBOARD_USAGE_PAGE,
            usUsage: KEYBOARD_USAGE,
            dwFlags: RIDEV_REMOVE,
            hwndTarget: std::ptr::null_mut(),
        }];
        if let Ok(size) = u32::try_from(std::mem::size_of::<RAWINPUTDEVICE>()) {
            // SAFETY: RIDEV_REMOVE must use a null hwndTarget per RegisterRawInputDevices.
            unsafe {
                let _ = RegisterRawInputDevices(devices.as_ptr(), 1, size);
            }
        }
    }

    pub(super) fn run(app: &SavantElite, duration_secs: u64) -> Result<()> {
        STOP.store(false, Ordering::SeqCst);
        let play_paths = list_play_device_paths()?;
        if play_paths.is_empty() {
            return Err(no_play_device_error());
        }
        app.verbose(&format!(
            "Found {} Play Raw Input keyboard path(s)",
            play_paths.len()
        ));
        for path in &play_paths {
            app.verbose(&format!("Play device: {path}"));
        }

        let class_name = wide(CLASS_NAME);
        let window_name = wide("savant-monitor");
        // SAFETY: null module name returns the handle of the current process image.
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            return Err(anyhow!(
                "GetModuleHandleW failed (GetLastError={})",
                last_error()
            ));
        }

        let class = WNDCLASSEXW {
            cbSize: u32::try_from(std::mem::size_of::<WNDCLASSEXW>())
                .context("WNDCLASSEXW size does not fit in u32")?,
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        // SAFETY: class points at a valid WNDCLASSEXW for this process.
        let atom = unsafe { RegisterClassExW(&class) };
        if atom == 0 && last_error() != 1410 {
            return Err(anyhow!(
                "RegisterClassExW failed (GetLastError={})",
                last_error()
            ));
        }

        // Hidden top-level window (null parent). HWND_MESSAGE windows do not
        // reliably receive WM_INPUT even with RIDEV_INPUTSINK.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                window_name.as_ptr(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err(anyhow!(
                "CreateWindowExW failed (GetLastError={}). \
Could not create the hidden Raw Input window.",
                last_error()
            ));
        }
        // SAFETY: hide the zero-size top-level window; it must remain a real HWND.
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
        app.verbose(&format!(
            "Hidden Raw Input hwnd={hwnd:p} (top-level, not HWND_MESSAGE)"
        ));

        let mut session = RawInputSession {
            app,
            path_cache: HashMap::new(),
            held_modifiers: Vec::new(),
            wm_input_count: 0,
            accepted_count: 0,
            filtered_count: 0,
            size_query_fail: 0,
            data_copy_fail: 0,
            short_header: 0,
            short_keyboard: 0,
            non_keyboard: 0,
        };
        // SAFETY: userdata pointer is the stack session, valid until DestroyWindow returns.
        unsafe {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                std::ptr::addr_of_mut!(session) as isize,
            );
        }

        // SAFETY: handler is a valid PHANDLER_ROUTINE; TRUE adds it.
        let handler_installed =
            unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), TRUE) } != FALSE;
        if !handler_installed {
            app.verbose(&format!(
                "SetConsoleCtrlHandler failed (GetLastError={}); default Ctrl+C still terminates",
                last_error()
            ));
        }

        let registered = register_sink(hwnd);
        if let Err(error) = registered {
            cleanup(hwnd, instance, &class_name, handler_installed);
            return Err(error);
        }
        app.verbose(&format!(
            "RegisterRawInputDevices requested flags=0x{:08X} (INPUTSINK|DEVNOTIFY; not NOLEGACY)",
            requested_raw_input_flags()
        ));
        log_registered_devices(app, hwnd);

        app.print_monitor_banner(
            "Watching pedal taps. Host keyboard keys are ignored.",
            duration_secs,
        );
        app.verbose("Starting Windows Raw Input message loop...");

        let start = Instant::now();
        let mut msg = MSG::default();
        loop {
            if duration_secs > 0 && start.elapsed().as_secs() >= duration_secs {
                break;
            }
            if STOP.load(Ordering::SeqCst) {
                break;
            }
            // SAFETY: thread message queue; hwnd=null receives WM_QUIT as well as window messages.
            let had_message =
                unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) };
            if had_message != FALSE {
                if msg.message == WM_QUIT {
                    break;
                }
                // SAFETY: standard translate/dispatch for a message retrieved by PeekMessageW.
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            } else {
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        if session.wm_input_count == 0 {
            app.verbose(
                "Raw Input summary: wm_input=0 accepted=0 filtered=0 (no WM_INPUT delivered to the hidden window)",
            );
        } else {
            app.verbose(&format!(
                "Raw Input summary: wm_input={} accepted={} filtered={} size_query_fail={} data_copy_fail={} short_header={} short_keyboard={} non_keyboard={}",
                session.wm_input_count,
                session.accepted_count,
                session.filtered_count,
                session.size_query_fail,
                session.data_copy_fail,
                session.short_header,
                session.short_keyboard,
                session.non_keyboard
            ));
        }

        cleanup(hwnd, instance, &class_name, handler_installed);
        app.print_monitor_complete();
        Ok(())
    }

    fn cleanup(
        hwnd: HWND,
        instance: windows_sys::Win32::Foundation::HINSTANCE,
        class_name: &[u16],
        handler_installed: bool,
    ) {
        unregister_sink();
        if !hwnd.is_null() {
            // SAFETY: hwnd was created by this monitor and is still valid.
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let _ = DestroyWindow(hwnd);
            }
        }
        // SAFETY: class was registered by this process for the monitor window.
        unsafe {
            let _ = UnregisterClassW(class_name.as_ptr(), instance);
        }
        if handler_installed {
            // SAFETY: FALSE removes the handler previously added by this monitor.
            unsafe {
                let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), FALSE);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::usb_hid;

    #[test]
    fn play_device_path_matches_case_insensitively() {
        assert!(is_play_raw_device_path(
            r"\\?\HID#VID_05F3&PID_030C#6&abc#{4d1e55b2-f16f-11cf-88cb-001111000030}"
        ));
        assert!(is_play_raw_device_path(
            r"\\?\hid#vid_05f3&pid_030c#6&abc#{4d1e55b2-f16f-11cf-88cb-001111000030}"
        ));
        assert!(is_play_raw_device_path(r"//?/hid#Vid_05f3&Pid_030c#6&abc"));
        assert_eq!(
            normalize_raw_device_path(r"//?/hid#vid_05f3&pid_030c"),
            r"\\?\HID#VID_05F3&PID_030C"
        );
    }

    #[test]
    fn play_device_path_rejects_other_keyboards() {
        assert!(!is_play_raw_device_path(
            r"\\?\HID#VID_05F3&PID_0232#6&abc#{4d1e55b2-f16f-11cf-88cb-001111000030}"
        ));
        assert!(!is_play_raw_device_path(
            r"\\?\HID#VID_046D&PID_C52B#7&def#{4d1e55b2-f16f-11cf-88cb-001111000030}"
        ));
        assert!(!is_play_raw_device_path(r"\\?\HID#VID_05F3&PID_030D#6&abc"));
        assert!(!is_play_raw_device_path(""));
        assert!(!is_play_raw_device_path("VID_05F3"));
        assert!(!is_play_raw_device_path("PID_030C"));
    }

    fn write_u16_le(buf: &mut [u8], offset: usize, value: u16) {
        buf[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32_le(buf: &mut [u8], offset: usize, value: u32) {
        buf[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_ptr_le(buf: &mut [u8], offset: usize, pointer_width: usize, value: u64) {
        match pointer_width {
            4 => write_u32_le(buf, offset, value as u32),
            8 => buf[offset..offset + 8].copy_from_slice(&value.to_le_bytes()),
            other => panic!("unsupported pointer width {other}"),
        }
    }

    fn keyboard_rawinput_bytes(pointer_width: usize, vkey: u16, flags: u16) -> Vec<u8> {
        let size = raw_keyboard_event_byte_size(pointer_width);
        let mut buf = vec![0u8; size];
        write_u32_le(&mut buf, 0, RIM_TYPE_KEYBOARD);
        write_u32_le(&mut buf, 4, u32::try_from(size).expect("size fits u32"));
        write_ptr_le(&mut buf, 8, pointer_width, 0x0000_05F3_030C);
        write_ptr_le(&mut buf, 8 + pointer_width, pointer_width, 0);
        let kb = rawinput_header_byte_size(pointer_width);
        write_u16_le(&mut buf, kb, 0x001E);
        write_u16_le(&mut buf, kb + 2, flags);
        write_u16_le(&mut buf, kb + 6, vkey);
        write_u32_le(&mut buf, kb + 8, 0x0100);
        buf
    }

    #[test]
    fn x64_keyboard_payload_parses_without_full_rawinput_union() {
        assert_eq!(rawinput_header_byte_size(8), 24);
        assert_eq!(raw_keyboard_event_byte_size(8), 40);
        assert!(
            raw_keyboard_event_byte_size(8) < RAWINPUT_X64_UNION_MAX_SIZE,
            "the hardware failure was 40 copied bytes vs sizeof(RAWINPUT)=48"
        );
        let bytes = keyboard_rawinput_bytes(8, VK_A, 0);
        assert_eq!(bytes.len(), 40);
        let (header, keyboard) = parse_raw_keyboard_event(&bytes, 8).expect("x64 keyboard");
        assert_eq!(header.dw_type, RIM_TYPE_KEYBOARD);
        assert_eq!(header.h_device, 0x0000_05F3_030C);
        assert_eq!(keyboard.vkey, VK_A);
        assert_eq!(keyboard.flags, 0);
        assert_eq!(keyboard.make_code, 0x001E);
    }

    #[test]
    fn x86_keyboard_payload_parses_at_32_bytes() {
        assert_eq!(rawinput_header_byte_size(4), 16);
        assert_eq!(raw_keyboard_event_byte_size(4), 32);
        let bytes = keyboard_rawinput_bytes(4, VK_A, RAW_KEY_BREAK);
        assert_eq!(bytes.len(), 32);
        let (header, keyboard) = parse_raw_keyboard_event(&bytes, 4).expect("x86 keyboard");
        assert_eq!(header.dw_type, RIM_TYPE_KEYBOARD);
        assert_eq!(keyboard.vkey, VK_A);
        assert_eq!(keyboard.flags, RAW_KEY_BREAK);
    }

    #[test]
    fn rawinput_parse_reports_short_header_keyboard_and_non_keyboard() {
        assert_eq!(
            parse_raw_keyboard_event(&[0u8; 8], 8),
            Err(RawInputParseError::ShortHeader)
        );
        let mut short_kb = keyboard_rawinput_bytes(8, VK_A, 0);
        short_kb.truncate(rawinput_header_byte_size(8) + 8);
        assert_eq!(
            parse_raw_keyboard_event(&short_kb, 8),
            Err(RawInputParseError::ShortKeyboard)
        );
        let mut mouse = keyboard_rawinput_bytes(8, VK_A, 0);
        write_u32_le(&mut mouse, 0, 0);
        assert_eq!(
            parse_raw_keyboard_event(&mouse, 8),
            Err(RawInputParseError::NonKeyboard { dw_type: 0 })
        );
    }

    #[test]
    fn x64_padded_union_buffer_still_reads_keyboard_arm() {
        let mut padded = keyboard_rawinput_bytes(8, VK_CONTROL, 0);
        padded.resize(RAWINPUT_X64_UNION_MAX_SIZE, 0);
        let (_header, keyboard) =
            parse_raw_keyboard_event(&padded, 8).expect("padded x64 keyboard");
        assert_eq!(keyboard.vkey, VK_CONTROL);
    }

    #[test]
    fn raw_key_phase_classifies_press_and_release() {
        assert_eq!(classify_raw_key_phase(0), KeyPhase::Press);
        assert_eq!(classify_raw_key_phase(RAW_KEY_BREAK), KeyPhase::Release);
        assert_eq!(classify_raw_key_phase(RAW_KEY_BREAK | 2), KeyPhase::Release);
        assert_eq!(classify_raw_key_phase(2), KeyPhase::Press);
    }

    #[test]
    fn decode_virtual_key_reports_lowercase_a_and_ctrl() {
        let letter = decode_virtual_key(VK_A).expect("VK_A");
        assert_eq!(letter.name, "a");
        assert!(!letter.is_modifier);

        let ctrl = decode_virtual_key(VK_CONTROL).expect("VK_CONTROL");
        assert_eq!(ctrl.name, "Ctrl");
        assert!(ctrl.is_modifier);

        let lctrl = decode_virtual_key(VK_LCONTROL).expect("VK_LCONTROL");
        assert_eq!(lctrl.name, "Ctrl");
        assert!(lctrl.is_modifier);
    }

    #[test]
    fn decode_virtual_key_reports_function_navigation_and_keypad() {
        assert_eq!(decode_virtual_key(VK_F1).expect("F1").name, "F1");
        assert_eq!(decode_virtual_key(VK_F24).expect("F24").name, "F24");
        assert_eq!(decode_virtual_key(0x7B).expect("VK_F12").name, "F12");
        assert_eq!(
            decode_virtual_key(VK_SNAPSHOT).expect("PrintScreen").name,
            "PrintScreen"
        );
        assert_eq!(
            decode_virtual_key(VK_SCROLL).expect("ScrollLock").name,
            "ScrollLock"
        );
        assert_eq!(decode_virtual_key(VK_PAUSE).expect("Pause").name, "Pause");
        assert_eq!(
            decode_virtual_key(VK_INSERT).expect("Insert").name,
            "Insert"
        );
        assert_eq!(decode_virtual_key(VK_HOME).expect("Home").name, "Home");
        assert_eq!(decode_virtual_key(VK_PRIOR).expect("PageUp").name, "PageUp");
        assert_eq!(
            decode_virtual_key(VK_DELETE).expect("Delete").name,
            "Delete"
        );
        assert_eq!(decode_virtual_key(VK_END).expect("End").name, "End");
        assert_eq!(
            decode_virtual_key(VK_NEXT).expect("PageDown").name,
            "PageDown"
        );
        assert_eq!(decode_virtual_key(VK_LEFT).expect("Left").name, "Left");
        assert_eq!(decode_virtual_key(VK_RIGHT).expect("Right").name, "Right");
        assert_eq!(decode_virtual_key(VK_UP).expect("Up").name, "Up");
        assert_eq!(decode_virtual_key(VK_DOWN).expect("Down").name, "Down");
        assert_eq!(
            decode_virtual_key(VK_NUMLOCK).expect("NumLock").name,
            "NumLock"
        );
        assert_eq!(
            decode_virtual_key(VK_DIVIDE).expect("KeypadDivide").name,
            "KeypadDivide"
        );
        assert_eq!(
            decode_virtual_key(VK_ADD).expect("KeypadAdd").name,
            "KeypadAdd"
        );
        assert_eq!(
            decode_virtual_key(VK_NUMPAD0).expect("Keypad0").name,
            "Keypad0"
        );
        assert_eq!(
            decode_virtual_key(VK_NUMPAD9).expect("Keypad9").name,
            "Keypad9"
        );
        assert_eq!(
            decode_virtual_key(VK_DECIMAL).expect("KeypadDecimal").name,
            "KeypadDecimal"
        );
        assert_eq!(decode_virtual_key(0x0D).expect("Enter").name, "Enter");
        assert_eq!(
            decode_virtual_key(VK_APPS).expect("Application").name,
            "Application"
        );
        for vkey in VK_F1..=VK_F24 {
            let decoded = decode_virtual_key(vkey).expect("Fn");
            assert!(
                decoded.name.starts_with('F'),
                "F-key {vkey:#04X} should be named, got {}",
                decoded.name
            );
            assert!(!decoded.is_modifier);
        }
    }

    #[test]
    fn modifier_state_formats_ctrl_plus_a() {
        let mut held = Vec::new();
        apply_modifier_state(&mut held, "Ctrl", true, KeyPhase::Press);
        assert_eq!(held, ["Ctrl"]);
        assert_eq!(format_key_display(&held, "Ctrl", true), "Ctrl");
        assert_eq!(format_key_display(&held, "a", false), "Ctrl+a");

        apply_modifier_state(&mut held, "a", false, KeyPhase::Press);
        assert_eq!(held, ["Ctrl"]);
        apply_modifier_state(&mut held, "Ctrl", true, KeyPhase::Release);
        assert!(held.is_empty());
        assert_eq!(format_key_display(&held, "a", false), "a");
    }

    #[test]
    fn normalize_boot_keyboard_report_too_short() {
        let data = [0u8; 7]; // Less than 8 bytes
        assert!(normalize_boot_keyboard_report(&data).is_none());
    }

    #[test]
    fn normalize_boot_keyboard_report_exact_8_bytes() {
        let data = [usb_hid::MOD_LEFT_GUI, 0, usb_hid::KEY_C, 0, 0, 0, 0, 0];
        let report = normalize_boot_keyboard_report(&data).unwrap();
        assert_eq!(report, data);
    }

    #[test]
    fn normalize_boot_keyboard_report_all_zeros() {
        let data = [0u8; 8];
        let report = normalize_boot_keyboard_report(&data).unwrap();
        assert_eq!(report, data);
    }

    #[test]
    fn normalize_boot_keyboard_report_all_keys_pressed() {
        // Modifier + 6 simultaneous keys (max for boot protocol)
        let data = [0xFF, 0, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        let report = normalize_boot_keyboard_report(&data).unwrap();
        assert_eq!(report, data);
    }

    #[test]
    fn normalize_boot_keyboard_report_prefixed() {
        let data = [0, usb_hid::MOD_LEFT_GUI, 0, usb_hid::KEY_C, 0, 0, 0, 0, 0];
        let report = normalize_boot_keyboard_report(&data).unwrap();
        assert_eq!(
            report,
            [usb_hid::MOD_LEFT_GUI, 0, usb_hid::KEY_C, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn normalize_boot_keyboard_report_padded() {
        let mut data = [0u8; 64];
        data[..8].copy_from_slice(&[0, 0, usb_hid::KEY_A, 0, 0, 0, 0, 0]);
        let report = normalize_boot_keyboard_report(&data).unwrap();
        assert_eq!(report, [0, 0, usb_hid::KEY_A, 0, 0, 0, 0, 0]);

        let mut prefixed = [0u8; 64];
        prefixed[..9].copy_from_slice(&[0, 0, 0, usb_hid::KEY_A, 0, 0, 0, 0, 0]);
        let report = normalize_boot_keyboard_report(&prefixed).unwrap();
        assert_eq!(report, [0, 0, usb_hid::KEY_A, 0, 0, 0, 0, 0]);
    }
}
