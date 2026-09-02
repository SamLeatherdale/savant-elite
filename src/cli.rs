//! CLI definitions, JSON output types, application facade, and dispatch.

use anyhow::{anyhow, Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use rich_rust::prelude::*;
use serde::Serialize;
use std::time::Duration;

use crate::config::PedalConfig;
use crate::protocol::{Pedal, ProgramAction, KINESIS_VID, PROGRAMMING_PID, SAVANT_ELITE_PID};
use crate::transport::{
    new_hid_api, prepare_erase, prepare_program, write_programming_request6,
    write_programming_request8, PreparedErase, PreparedProgram, DEFAULT_USB_TIMEOUT_MS,
};

// JSON output structures for --json flag
#[derive(Serialize)]
pub(crate) struct JsonDeviceInterface {
    pub mode: String,
    pub vid: String,
    pub pid: String,
    pub interface: i32,
    pub usage_page: String,
    pub usage: String,
}

#[derive(Serialize)]
pub(crate) struct JsonConfig {
    pub source: String,
    pub path: String,
    pub left: String,
    pub middle: String,
    pub right: String,
}

#[derive(Serialize)]
pub(crate) struct JsonInfoOutput {
    pub device: JsonDeviceInfo,
    pub config: Option<JsonConfig>,
}

#[derive(Serialize)]
pub(crate) struct JsonDeviceInfo {
    pub detected: bool,
    pub mode: Option<String>,
    pub vid: String,
    pub pid: Option<String>,
    pub path: Option<String>,
    pub serial: Option<String>,
    pub interfaces: Vec<JsonDeviceInterface>,
}

#[derive(Serialize)]
pub(crate) struct JsonStatusDevice {
    pub mode: String,
    pub pid: String,
    pub location: String,
}

#[derive(Serialize)]
pub(crate) struct JsonStatusOutput {
    pub detected: bool,
    pub mode: Option<String>,
    pub devices: Vec<JsonStatusDevice>,
    pub ready_to_program: bool,
}

#[derive(Serialize)]
pub(crate) struct JsonPreset {
    pub name: String,
    pub description: String,
    pub left: String,
    pub middle: String,
    pub right: String,
}

#[derive(Serialize)]
pub(crate) struct JsonPresetListOutput {
    pub presets: Vec<JsonPreset>,
}

#[derive(Serialize)]
pub(crate) struct JsonProfile {
    pub name: String,
    pub left: String,
    pub middle: String,
    pub right: String,
}

#[derive(Serialize)]
pub(crate) struct JsonProfileListOutput {
    pub profiles: Vec<JsonProfile>,
    pub profiles_dir: String,
}

#[derive(Serialize)]
pub(crate) struct JsonProfileSaveOutput {
    pub success: bool,
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub(crate) struct JsonProfileDeleteOutput {
    pub success: bool,
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct JsonConfigCheckError {
    pub line: Option<usize>,
    pub field: Option<String>,
    pub value: Option<String>,
    pub error: String,
}

#[derive(Serialize)]
pub(crate) struct JsonConfigCheckParsedKey {
    pub action: String,
    pub modifier_hex: String,
    pub key_hex: String,
}

#[derive(Serialize)]
pub(crate) struct JsonConfigCheckOutput {
    pub valid: bool,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<JsonConfigCheckParsedKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middle: Option<JsonConfigCheckParsedKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<JsonConfigCheckParsedKey>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<JsonConfigCheckError>,
}

#[derive(Serialize)]
pub(crate) struct JsonDoctorCheck {
    pub name: String,
    pub status: String, // "pass", "warn", "fail"
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct JsonDoctorOutput {
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub checks: Vec<JsonDoctorCheck>,
    pub summary: JsonDoctorSummary,
}

#[derive(Serialize)]
pub(crate) struct JsonDoctorSummary {
    pub total: usize,
    pub passed: usize,
    pub warnings: usize,
    pub failed: usize,
    pub healthy: bool,
}

#[derive(Serialize)]
pub(crate) struct JsonProgramSetup {
    #[serde(rename = "bmRequestType")]
    pub bm_request_type: String,
    pub direction: String,
    #[serde(rename = "type")]
    pub transfer_type: String,
    pub recipient: String,
    #[serde(rename = "bRequest")]
    pub b_request: u8,
    #[serde(rename = "wValue")]
    pub w_value: u16,
    #[serde(rename = "wIndex")]
    pub w_index: u16,
    #[serde(rename = "wLength")]
    pub w_length: usize,
}

#[derive(Serialize)]
pub(crate) struct JsonProgramOutput {
    pub dry_run: bool,
    pub wrote: bool,
    pub pedal: String,
    pub action: String,
    pub setup: JsonProgramSetup,
    pub payload_hex: String,
    pub payload: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_written: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct JsonEraseOutput {
    pub dry_run: bool,
    pub wrote: bool,
    pub setup: JsonProgramSetup,
    pub payload_hex: String,
    pub payload: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_written: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<String>,
}

#[derive(Parser)]
#[command(name = "savant")]
#[command(version)]
#[command(about = "Windows-first CLI for the Kinesis Savant Elite / X-keys USB foot pedal")]
#[command(
    long_about = "Windows-first Rust CLI for the discontinued Kinesis Savant Elite / X-keys USB foot pedal.\n\nOn Windows, programming writes one pedal mapping at a time and Play-mode monitoring uses Raw Input. macOS still builds; programming there is unverified."
)]
pub struct Cli {
    /// Enable verbose output for debugging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output in JSON format for scripting
    #[arg(long, global = true)]
    pub json: bool,

    /// USB operation timeout in milliseconds [default: 500]
    #[arg(long, global = true, value_name = "MS", value_parser = clap::value_parser!(u64).range(100..=600000))]
    pub timeout: Option<u64>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Detect and show info about connected Savant Elite pedals
    Info,

    /// Watch Play-mode pedal taps
    Monitor {
        /// Duration in seconds (0 = infinite)
        #[arg(short, long, default_value = "30")]
        duration: u64,
    },

    /// Program one pedal mapping (requires Programming mode)
    Program {
        /// Pedal to program (`a`, `b`, or `c`)
        #[arg(long, value_name = "a|b|c")]
        pedal: String,

        /// Action: key chords (`a`, `ctrl+a`, `a,b`), one modifier (`ctrl`), mouse (`left-click`), or `clear`
        #[arg(long, value_name = "ACTION")]
        action: String,

        /// Preview the mapping without writing to the pedal
        #[arg(long)]
        dry_run: bool,

        /// Confirm a real write (required; there is no default write)
        #[arg(long)]
        yes: bool,
    },

    /// Erase every pedal mapping (requires Programming mode)
    Erase {
        /// Preview the erase without writing to the pedal
        #[arg(long)]
        dry_run: bool,

        /// Confirm a real erase (required; there is no default write)
        #[arg(long)]
        yes: bool,
    },

    /// Check if the pedal is in Play or Program mode
    Status,

    /// List all valid key names and modifiers
    Keys {
        /// Output in JSON format for scripting
        #[arg(long)]
        json: bool,
    },

    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// List or show built-in pedal mapping names (this command does not write the pedal)
    Preset {
        /// Preset name to show (this command does not write the pedal; use savant program)
        #[arg(value_name = "NAME")]
        name: Option<String>,

        /// List all available presets
        #[arg(long, short = 'l')]
        list: bool,

        /// Show details of a specific preset
        #[arg(long)]
        show: bool,

        /// This command does not write the pedal; use savant program
        #[arg(long)]
        dry_run: bool,
    },

    /// Manage saved pedal configuration profiles
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },

    /// Run system diagnostics to identify configuration issues
    Doctor,
}

/// Subcommands for the config command
#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Save current configuration as a named profile
    Save {
        /// Name for the profile (alphanumeric, hyphens, underscores)
        name: String,

        /// Overwrite existing profile without prompting
        #[arg(long, short = 'f')]
        force: bool,
    },

    /// Load a saved profile (this command does not write the pedal)
    Load {
        /// Name of the profile to load
        name: String,

        /// This command does not write the pedal; use savant program
        #[arg(long)]
        dry_run: bool,
    },

    /// List all saved profiles
    List,

    /// Show contents of a saved profile
    Show {
        /// Name of the profile to show
        name: String,
    },

    /// Delete a saved profile
    Delete {
        /// Name of the profile to delete
        name: String,

        /// Delete without confirmation
        #[arg(long, short = 'f')]
        force: bool,
    },

    /// Validate a configuration file
    Check {
        /// Path to config file (defaults to current config)
        #[arg(value_name = "FILE")]
        file: Option<String>,
    },

    /// Show configuration history (automatic backups)
    History,

    /// Restore a previous configuration from history
    Restore {
        /// Backup number to restore (1 = most recent)
        number: usize,

        /// This command does not write the pedal; use savant program
        #[arg(long)]
        apply: bool,
    },
}

pub struct SavantElite {
    pub(crate) console: Console,
    pub(crate) verbose: bool,
    pub(crate) json_output: bool,
    pub(crate) timeout_ms: u64,
}

impl SavantElite {
    pub fn new(verbose: bool, json_output: bool, timeout_ms: u64) -> Result<Self> {
        Ok(Self {
            console: Console::new(),
            verbose,
            json_output,
            timeout_ms,
        })
    }

    /// Print verbose output to stderr if verbose mode is enabled
    pub(crate) fn verbose(&self, msg: &str) {
        if self.verbose {
            eprintln!("[verbose] {}", msg);
        }
    }

    /// Print verbose hex data to stderr if verbose mode is enabled
    #[cfg(not(windows))]
    pub(crate) fn verbose_hex(&self, label: &str, data: &[u8]) {
        if self.verbose {
            let hex: Vec<String> = data.iter().map(|b| format!("{:02X}", b)).collect();
            if data.len() <= 16 {
                eprintln!("[verbose] {}: [{}]", label, hex.join(" "));
            } else {
                eprintln!(
                    "[verbose] {}: [{}...] ({} bytes)",
                    label,
                    hex[..16].join(" "),
                    data.len()
                );
            }
        }
    }

    pub(crate) fn print_banner(&self) {
        self.console.print("");
        self.console
            .print("[bold #ff6b6b]╔══════════════════════════════════════════════════════════╗[/]");
        self.console.print(
            "[bold #ff6b6b]║[/]                                                          [bold #ff6b6b]║[/]",
        );
        self.console.print(
            "[bold #ff6b6b]║[/]   [bold #4ecdc4]SAVANT ELITE[/]  -  [bold #ffe66d]Kinesis Foot Pedal Programmer[/]         [bold #ff6b6b]║[/]",
        );
        self.console.print(
            "[bold #ff6b6b]║[/]                                                          [bold #ff6b6b]║[/]",
        );
        self.console
            .print("[bold #ff6b6b]╚══════════════════════════════════════════════════════════╝[/]");
        self.console.print("");
    }

    pub(crate) fn print_pedal_visualization(&self, left: &str, middle: &str, right: &str) {
        // Helper to center text in a fixed width
        fn center(s: &str, width: usize) -> String {
            let len = s.chars().count();
            if len >= width {
                s.chars().take(width).collect()
            } else {
                let pad = width - len;
                let left_pad = pad / 2;
                let right_pad = pad - left_pad;
                format!("{}{}{}", " ".repeat(left_pad), s, " ".repeat(right_pad))
            }
        }

        // Format key action for display (e.g., "cmd+c" -> "⌘C")
        fn format_key(s: &str) -> String {
            let s = s.to_lowercase();
            let parts: Vec<&str> = s.split('+').collect();
            let mut result = String::new();

            for (i, part) in parts.iter().enumerate() {
                let part = part.trim();
                if i < parts.len() - 1 {
                    // Modifier
                    match part {
                        "cmd" | "command" | "gui" | "meta" | "super" => result.push('⌘'),
                        "ctrl" | "control" => result.push('⌃'),
                        "shift" => result.push('⇧'),
                        "alt" | "option" | "opt" => result.push('⌥'),
                        _ => result.push_str(part),
                    }
                } else {
                    // Key - uppercase for display
                    result.push_str(&part.to_uppercase());
                }
            }
            result
        }

        let left_key = format_key(left);
        let middle_key = format_key(middle);
        let right_key = format_key(right);

        // Create centered key displays (max 7 chars for the box interior)
        let left_display = center(&left_key, 7);
        let middle_display = center(&middle_key, 7);
        let right_display = center(&right_key, 7);

        self.console.print("");
        self.console
            .print("[bold #9b59b6]┌──────────────────────────────────────────────────────────┐[/]");
        self.console.print(
            "[bold #9b59b6]│[/]                 [bold white]YOUR PEDAL CONFIGURATION[/]                 [bold #9b59b6]│[/]",
        );
        self.console
            .print("[bold #9b59b6]└──────────────────────────────────────────────────────────┘[/]");
        self.console.print("");

        // Top of pedals
        self.console.print(
            "       [#e74c3c]╭─────────────╮[/]  [#f39c12]╭─────────────╮[/]  [#2ecc71]╭─────────────╮[/]",
        );
        self.console.print(
            "       [#e74c3c]│[/]             [#e74c3c]│[/]  [#f39c12]│[/]             [#f39c12]│[/]  [#2ecc71]│[/]             [#2ecc71]│[/]",
        );

        // Pedal labels
        self.console.print(
            "       [#e74c3c]│[/]  [bold #e74c3c]◀ LEFT[/]    [#e74c3c]│[/]  [#f39c12]│[/]  [bold #f39c12]● MIDDLE[/]  [#f39c12]│[/]  [#2ecc71]│[/]  [bold #2ecc71]RIGHT ▶[/]   [#2ecc71]│[/]",
        );

        self.console.print(
            "       [#e74c3c]│[/]             [#e74c3c]│[/]  [#f39c12]│[/]             [#f39c12]│[/]  [#2ecc71]│[/]             [#2ecc71]│[/]",
        );

        // Key box top
        self.console.print(
            "       [#e74c3c]│[/]  [bold #e74c3c]┌───────┐[/]  [#e74c3c]│[/]  [#f39c12]│[/]  [bold #f39c12]┌───────┐[/]  [#f39c12]│[/]  [#2ecc71]│[/]  [bold #2ecc71]┌───────┐[/]  [#2ecc71]│[/]",
        );

        // Key values
        self.console.print(&format!(
            "       [#e74c3c]│[/]  [bold #e74c3c]│[/][bold white]{}[/][bold #e74c3c]│[/]  [#e74c3c]│[/]  [#f39c12]│[/]  [bold #f39c12]│[/][bold white]{}[/][bold #f39c12]│[/]  [#f39c12]│[/]  [#2ecc71]│[/]  [bold #2ecc71]│[/][bold white]{}[/][bold #2ecc71]│[/]  [#2ecc71]│[/]",
            left_display, middle_display, right_display
        ));

        // Key box bottom
        self.console.print(
            "       [#e74c3c]│[/]  [bold #e74c3c]└───────┘[/]  [#e74c3c]│[/]  [#f39c12]│[/]  [bold #f39c12]└───────┘[/]  [#f39c12]│[/]  [#2ecc71]│[/]  [bold #2ecc71]└───────┘[/]  [#2ecc71]│[/]",
        );

        self.console.print(
            "       [#e74c3c]│[/]             [#e74c3c]│[/]  [#f39c12]│[/]             [#f39c12]│[/]  [#2ecc71]│[/]             [#2ecc71]│[/]",
        );

        // Bottom of pedals
        self.console.print(
            "       [#e74c3c]╰─────────────╯[/]  [#f39c12]╰─────────────╯[/]  [#2ecc71]╰─────────────╯[/]",
        );

        // Pedal "stems"
        self.console.print(
            "            [#e74c3c]│[/]                  [#f39c12]│[/]                  [#2ecc71]│[/]",
        );
        self.console
            .print("       [dim]═════╧══════════════════╧══════════════════╧═════[/]");
        self.console.print("");
    }

    pub(crate) fn find_device(&self) -> Result<()> {
        self.verbose("Initializing HID API...");
        let api = new_hid_api().context("Failed to initialize HID API")?;
        self.verbose("HID API initialized successfully");

        // (mode, vid, pid, path, serial, interface, usage_page, usage)
        type DeviceInfo = (String, String, String, String, String, i32, u16, u16);
        let mut found_any = false;
        let mut devices_info: Vec<DeviceInfo> = Vec::new();

        self.verbose("Enumerating HID devices...");
        for device in api.device_list() {
            if device.vendor_id() == KINESIS_VID
                && (device.product_id() == SAVANT_ELITE_PID
                    || device.product_id() == PROGRAMMING_PID)
            {
                found_any = true;
                let mode = if device.product_id() == PROGRAMMING_PID {
                    "PROGRAM".to_string()
                } else {
                    "PLAY".to_string()
                };
                self.verbose(&format!(
                    "Found Savant Elite ({} mode): VID={:#06X} PID={:#06X} interface={}",
                    mode,
                    device.vendor_id(),
                    device.product_id(),
                    device.interface_number()
                ));
                devices_info.push((
                    mode,
                    format!("0x{:04X}", device.vendor_id()),
                    format!("0x{:04X}", device.product_id()),
                    device.path().to_string_lossy().to_string(),
                    device.serial_number().unwrap_or("N/A").to_string(),
                    device.interface_number(),
                    device.usage_page(),
                    device.usage(),
                ));
            }
        }

        // Load config
        self.verbose(&format!(
            "Loading config from: {}",
            PedalConfig::config_path().display()
        ));
        let config = PedalConfig::load();
        if let Some(ref cfg) = config {
            self.verbose(&format!(
                "Config loaded: left={}, middle={}, right={}",
                cfg.left, cfg.middle, cfg.right
            ));
        } else {
            self.verbose("No saved config found");
        }

        // JSON output mode
        if self.json_output {
            let mut interfaces: Vec<JsonDeviceInterface> = Vec::new();
            let mut seen_interfaces = std::collections::HashSet::new();

            for (mode, vid, pid, _path, _serial, iface, usage_page, usage) in &devices_info {
                if seen_interfaces.insert((*iface, *usage_page, *usage)) {
                    interfaces.push(JsonDeviceInterface {
                        mode: mode.to_lowercase(),
                        vid: vid.clone(),
                        pid: pid.clone(),
                        interface: *iface,
                        usage_page: format!("0x{:04X}", usage_page),
                        usage: format!("0x{:04X}", usage),
                    });
                }
            }

            let (path, serial) = devices_info.first().map_or((None, None), |d| {
                let serial = if d.4.is_empty() || d.4 == "N/A" {
                    None
                } else {
                    Some(d.4.clone())
                };
                (Some(d.3.clone()), serial)
            });

            let mode = devices_info.first().map(|d| d.0.to_lowercase());
            let pid = devices_info.first().map(|d| d.2.clone());

            let json_config = config.map(|cfg| JsonConfig {
                source: "file".to_string(),
                path: PedalConfig::config_path().display().to_string(),
                left: cfg.left,
                middle: cfg.middle,
                right: cfg.right,
            });

            let output = JsonInfoOutput {
                device: JsonDeviceInfo {
                    detected: found_any,
                    mode,
                    vid: format!("0x{:04X}", KINESIS_VID),
                    pid,
                    path,
                    serial,
                    interfaces,
                },
                config: json_config,
            };

            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        // Human-readable output
        self.print_banner();

        if found_any {
            self.console.print(
                "[bold #3498db]┌──────────────────────────────────────────────────────────┐[/]",
            );
            self.console.print(
                "[bold #3498db]│[/]  [bold #2ecc71]✓[/] [bold white]DEVICE DETECTED[/]                                       [bold #3498db]│[/]",
            );
            self.console.print(
                "[bold #3498db]└──────────────────────────────────────────────────────────┘[/]",
            );
            self.console.print("");

            let mut seen_modes = std::collections::HashSet::new();
            let mut in_program = false;
            let mut in_play = false;
            for (mode, vid, pid, path, serial, iface, usage_page, usage) in &devices_info {
                self.verbose(&format!(
                    "Interface {} {} VID={} PID={} usage=0x{:04X}:0x{:04X} path={} serial={}",
                    iface, mode, vid, pid, usage_page, usage, path, serial
                ));
                if !seen_modes.insert(mode.as_str()) {
                    continue;
                }
                if mode == "PROGRAM" {
                    in_program = true;
                    self.console.print(
                        "  Mode: [bold #e74c3c]Program[/]  [bold #2ecc71]Ready to program[/]",
                    );
                } else {
                    in_play = true;
                    self.console.print(
                        "  Mode: [bold #2ecc71]Play[/]  [#95a5a6]tap a pedal to send keys[/]",
                    );
                }
            }
            self.console.print("");
            if in_program {
                self.console.print(
                    "  Preview a mapping with [bold #f1c40f]savant program --pedal a --action a --dry-run[/]",
                );
            } else if in_play {
                self.console.print(
                    "  To write a mapping, switch to Program, unplug, and replug. To watch taps: [bold #f1c40f]savant monitor[/]",
                );
            }

            // Show current pedal configuration from saved config
            if let Some(cfg) = config {
                self.print_pedal_visualization(&cfg.left, &cfg.middle, &cfg.right);
            } else {
                self.console.print("");
                self.console.print(
                    "[bold #f39c12]┌──────────────────────────────────────────────────────────┐[/]",
                );
                self.console.print(
                    "[bold #f39c12]│[/]  [bold white]PEDAL CONFIGURATION UNKNOWN[/]                             [bold #f39c12]│[/]",
                );
                self.console.print(
                    "[bold #f39c12]└──────────────────────────────────────────────────────────┘[/]",
                );
                self.console.print("");
                self.console
                    .print("  [#95a5a6]Run[/] [bold #3498db]savant program --pedal a --action a --dry-run[/] [#95a5a6]to preview a mapping.[/]");
                self.console.print("");
                self.console.print(
                    "  [dim]Example:[/] [#95a5a6]savant program --pedal a --action a --yes[/]",
                );
            }
        } else {
            self.console.print(
                "[bold #e74c3c]┌──────────────────────────────────────────────────────────┐[/]",
            );
            self.console.print(
                "[bold #e74c3c]│[/]  [bold #e74c3c]✗[/] [bold white]NO DEVICE FOUND[/]                                       [bold #e74c3c]│[/]",
            );
            self.console.print(
                "[bold #e74c3c]└──────────────────────────────────────────────────────────┘[/]",
            );
            self.console.print("");
            self.console
                .print("  [#95a5a6]Make sure your Savant Elite is connected via USB.[/]");
        }

        self.console.print("");
        Ok(())
    }

    pub(crate) fn status(&self) -> Result<()> {
        // Check via libusb first (more reliable for programming mode)
        let mut found_play_usb = false;
        let mut found_program_usb = false;
        let mut libusb_error: Option<anyhow::Error> = None;
        let mut device_details: Vec<(String, String, String)> = Vec::new();

        match rusb::devices() {
            Ok(devices) => {
                for device in devices.iter() {
                    let desc = match device.device_descriptor() {
                        Ok(desc) => desc,
                        Err(_) => continue,
                    };
                    if desc.vendor_id() == KINESIS_VID {
                        match desc.product_id() {
                            SAVANT_ELITE_PID => {
                                found_play_usb = true;
                                device_details.push((
                                    "PLAY".to_string(),
                                    format!("0x{:04X}", SAVANT_ELITE_PID),
                                    format!(
                                        "Bus {:03} Device {:03}",
                                        device.bus_number(),
                                        device.address()
                                    ),
                                ));
                            }
                            PROGRAMMING_PID => {
                                found_program_usb = true;
                                let mut product = "Savant Elite".to_string();
                                let mut mfr = "Kinesis".to_string();

                                if let Ok(handle) = device.open() {
                                    if let Ok(langs) =
                                        handle.read_languages(Duration::from_millis(100))
                                    {
                                        if let Some(lang) = langs.first() {
                                            if let Ok(p) = handle.read_product_string(
                                                *lang,
                                                &desc,
                                                Duration::from_millis(100),
                                            ) {
                                                product = p;
                                            }
                                            if let Ok(m) = handle.read_manufacturer_string(
                                                *lang,
                                                &desc,
                                                Duration::from_millis(100),
                                            ) {
                                                mfr = m;
                                            }
                                        }
                                    }
                                }
                                device_details.push((
                                    "PROGRAM".to_string(),
                                    format!("0x{:04X}", PROGRAMMING_PID),
                                    format!("{mfr} - {product}"),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(e) => {
                libusb_error =
                    Some(anyhow!(e).context("Failed to enumerate USB devices via libusb"));
            }
        }

        // Also check HID (for play mode with interfaces)
        let api = new_hid_api().context("Failed to initialize HID API")?;
        let mut found_play_hid = false;
        let mut found_program_hid = false;

        for device_info in api.device_list() {
            if device_info.vendor_id() != KINESIS_VID {
                continue;
            }

            match device_info.product_id() {
                SAVANT_ELITE_PID if !found_play_usb && !found_play_hid => {
                    found_play_hid = true;
                    device_details.push((
                        "PLAY".to_string(),
                        format!("0x{:04X}", SAVANT_ELITE_PID),
                        format!("hidapi: {}", device_info.path().to_string_lossy()),
                    ));
                }
                PROGRAMMING_PID if !found_program_usb && !found_program_hid => {
                    found_program_hid = true;
                    device_details.push((
                        "PROGRAM".to_string(),
                        format!("0x{:04X}", PROGRAMMING_PID),
                        format!("hidapi: {}", device_info.path().to_string_lossy()),
                    ));
                }
                _ => {}
            }
        }

        let found_play = found_play_usb || found_play_hid;
        let found_program = found_program_usb || found_program_hid;

        // JSON output mode
        if self.json_output {
            let mode = if found_program {
                Some("program".to_string())
            } else if found_play {
                Some("play".to_string())
            } else {
                None
            };

            let devices: Vec<JsonStatusDevice> = device_details
                .iter()
                .map(|(m, pid, loc)| JsonStatusDevice {
                    mode: m.to_lowercase(),
                    pid: pid.clone(),
                    location: loc.clone(),
                })
                .collect();

            let output = JsonStatusOutput {
                detected: found_play || found_program,
                mode,
                devices,
                ready_to_program: found_program,
            };

            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        // Human-readable output
        self.print_banner();

        self.console.print(
            "[bold #1abc9c]┌─────────────────────────────────────────────────────────────────┐[/]",
        );
        self.console.print(
            "[bold #1abc9c]│[/]  [bold #f39c12]🔍[/] [bold white]DEVICE STATUS CHECK[/]                                        [bold #1abc9c]│[/]",
        );
        self.console.print(
            "[bold #1abc9c]└─────────────────────────────────────────────────────────────────┘[/]",
        );
        self.console.print("");

        if !found_play && !found_program {
            self.console.print(
                "  [bold #e74c3c]╭────────────────────────────────────────────────────────────╮[/]",
            );
            self.console.print(
                "  [bold #e74c3c]│[/]  [bold #e74c3c]✗[/]  [bold white]No Savant Elite device found[/]                           [bold #e74c3c]│[/]",
            );
            self.console.print(
                "  [bold #e74c3c]╰────────────────────────────────────────────────────────────╯[/]",
            );
            self.console.print("");
            self.console.print("  [bold #f39c12]Troubleshooting:[/]");
            self.console
                .print("    [#95a5a6]1.[/] Make sure the device is connected via USB");
            self.console
                .print("    [#95a5a6]2.[/] Try unplugging and replugging the cable");
        } else if found_play && !found_program {
            self.console.print(
                "  [bold #2ecc71]╭────────────────────────────────────────────────────────────╮[/]",
            );
            self.console.print(
                "  [bold #2ecc71]│[/]  [bold #2ecc71]●[/]  [bold white]Device is in[/] [bold #2ecc71]PLAY[/] [bold white]mode[/]                                 [bold #2ecc71]│[/]",
            );
            self.console.print(
                "  [bold #2ecc71]╰────────────────────────────────────────────────────────────╯[/]",
            );
            self.console.print("");

            for (mode, pid, location) in &device_details {
                self.verbose(&format!("Status detail: {} PID={} {}", mode, pid, location));
            }

            self.console
                .print("  [bold #f39c12]To program the pedal, switch to Program:[/]");
            self.console.print("");
            self.console
                .print("    [bold #3498db]1.[/] Flip the pedal over");
            self.console
                .print("    [bold #3498db]2.[/] Find the recessed switch near the Kinesis sticker");
            self.console.print(
                "    [bold #3498db]3.[/] Use a paperclip to flip it from [#2ecc71]Play[/] → [#e74c3c]Program[/]",
            );
            self.console
                .print("    [bold #3498db]4.[/] Unplug and replug the USB cable");
            self.console
                .print("    [bold #3498db]5.[/] Run [bold #f1c40f]savant status[/] to verify");
        } else if found_program {
            self.console.print(
                "  [bold #e74c3c]╭────────────────────────────────────────────────────────────╮[/]",
            );
            self.console.print(
                "  [bold #e74c3c]│[/]  [bold #e74c3c]◉[/]  [bold white]Device is in[/] [bold #e74c3c]PROGRAMMING[/] [bold white]mode[/]                          [bold #e74c3c]│[/]",
            );
            self.console.print(
                "  [bold #e74c3c]╰────────────────────────────────────────────────────────────╯[/]",
            );
            self.console.print("");

            for (mode, pid, info) in &device_details {
                self.verbose(&format!("Status detail: {} PID={} {}", mode, pid, info));
            }

            self.console
                .print("  [bold #2ecc71]✓[/] [bold white]Ready to program[/]");
            self.console.print("");
            self.console.print("  [#95a5a6]Example command:[/]");
            self.console
                .print("    [bold #f1c40f]savant program --pedal a --action a --dry-run[/]");
            self.console
                .print("    [bold #f1c40f]savant program --pedal a --action a --yes[/]");
        }

        if let Some(e) = libusb_error {
            self.verbose(&format!("USB scan failed: {e}"));
            self.console.print("");
            self.console.print(
                "  [bold #f39c12]⚠[/] Could not complete a USB scan. If the pedal is plugged in, unplug and replug it.",
            );
            if !cfg!(windows) {
                self.console
                    .print("  [#95a5a6]On some systems this scan needs elevated permissions.[/]");
            }
        }

        self.console.print("");
        Ok(())
    }

    /// List all valid key names and modifiers
    pub(crate) fn list_keys(&self, json_output: bool) -> Result<()> {
        // Data structures for key information
        #[derive(Serialize)]
        struct ModifierInfo {
            names: Vec<&'static str>,
            symbol: &'static str,
            description: &'static str,
        }

        #[derive(Serialize)]
        struct KeysOutput {
            modifiers: Vec<ModifierInfo>,
            keys: KeyCategories,
            mouse: Vec<&'static str>,
        }

        #[derive(Serialize)]
        struct KeyCategories {
            letters: Vec<&'static str>,
            numbers: Vec<&'static str>,
            function_keys: Vec<&'static str>,
            special: Vec<KeyAliases>,
            arrow_keys: Vec<&'static str>,
            punctuation: Vec<KeyAliases>,
        }

        #[derive(Serialize)]
        struct KeyAliases {
            names: Vec<&'static str>,
        }

        let modifiers = vec![
            ModifierInfo {
                names: vec!["ctrl", "control", "lctrl"],
                symbol: "⌃",
                description: "Left Control",
            },
            ModifierInfo {
                names: vec!["shift", "lshift"],
                symbol: "⇧",
                description: "Left Shift",
            },
            ModifierInfo {
                names: vec!["alt", "option", "lalt"],
                symbol: "⌥",
                description: "Left Alt",
            },
            ModifierInfo {
                names: vec!["gui", "win", "cmd", "lgui"],
                symbol: "⌘",
                description: "Left GUI / Windows / Command",
            },
            ModifierInfo {
                names: vec!["rctrl"],
                symbol: "⌃",
                description: "Right Control",
            },
            ModifierInfo {
                names: vec!["rshift"],
                symbol: "⇧",
                description: "Right Shift",
            },
            ModifierInfo {
                names: vec!["ralt"],
                symbol: "⌥",
                description: "Right Alt",
            },
            ModifierInfo {
                names: vec!["rgui", "rwin"],
                symbol: "⌘",
                description: "Right GUI / Windows",
            },
        ];

        let letters: Vec<&'static str> = vec![
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
            "r", "s", "t", "u", "v", "w", "x", "y", "z",
        ];

        let numbers: Vec<&'static str> = vec!["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

        let function_keys: Vec<&'static str> = vec![
            "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12",
        ];

        let special = vec![
            KeyAliases {
                names: vec!["enter", "return"],
            },
            KeyAliases {
                names: vec!["esc", "escape"],
            },
            KeyAliases {
                names: vec!["backspace"],
            },
            KeyAliases { names: vec!["tab"] },
            KeyAliases {
                names: vec!["space"],
            },
            KeyAliases {
                names: vec!["capslock"],
            },
            KeyAliases {
                names: vec!["printscreen", "print-screen", "prtsc"],
            },
            KeyAliases {
                names: vec!["scrolllock", "scroll-lock"],
            },
            KeyAliases {
                names: vec!["pause"],
            },
            KeyAliases {
                names: vec!["insert", "ins"],
            },
            KeyAliases {
                names: vec!["home"],
            },
            KeyAliases {
                names: vec!["pageup", "page-up", "pgup"],
            },
            KeyAliases {
                names: vec!["delete", "del", "deleteforward"],
            },
            KeyAliases { names: vec!["end"] },
            KeyAliases {
                names: vec!["pagedown", "page-down", "pgdn"],
            },
            KeyAliases {
                names: vec!["numlock", "num-lock"],
            },
            KeyAliases {
                names: vec!["application", "menu"],
            },
            KeyAliases {
                names: vec!["keypad-enter"],
            },
            KeyAliases {
                names: vec!["keypad-plus", "keypad-add"],
            },
            KeyAliases {
                names: vec!["keypad-divide", "keypad-multiply", "keypad-subtract"],
            },
            KeyAliases {
                names: vec![
                    "keypad-0",
                    "keypad-1",
                    "keypad-2",
                    "keypad-3",
                    "keypad-4",
                    "keypad-5",
                    "keypad-6",
                    "keypad-7",
                    "keypad-8",
                    "keypad-9",
                    "keypad-decimal",
                ],
            },
        ];

        let arrow_keys: Vec<&'static str> = vec!["up", "down", "left", "right"];

        let mouse: Vec<&'static str> = vec![
            "left-click",
            "right-click",
            "middle-click",
            "scroll-up",
            "scroll-down",
        ];

        let punctuation = vec![
            KeyAliases {
                names: vec!["minus", "-"],
            },
            KeyAliases {
                names: vec!["equal", "="],
            },
            KeyAliases {
                names: vec!["leftbracket", "["],
            },
            KeyAliases {
                names: vec!["rightbracket", "]"],
            },
            KeyAliases {
                names: vec!["backslash", "\\"],
            },
            KeyAliases {
                names: vec!["semicolon", ";"],
            },
            KeyAliases {
                names: vec!["quote", "'"],
            },
            KeyAliases {
                names: vec!["grave", "`"],
            },
            KeyAliases {
                names: vec!["comma", ","],
            },
            KeyAliases {
                names: vec!["period", "."],
            },
            KeyAliases {
                names: vec!["slash", "/"],
            },
        ];

        if json_output {
            let output = KeysOutput {
                modifiers,
                keys: KeyCategories {
                    letters,
                    numbers,
                    function_keys,
                    special,
                    arrow_keys,
                    punctuation,
                },
                mouse,
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            // Human-readable output
            self.console
                .print("[bold cyan]MODIFIERS[/] [dim](combine with + before key)[/]");
            self.console.print("");
            for m in &modifiers {
                let names = m.names.join(", ");
                self.console.print(&format!(
                    "  [green]{}[/]  →  {} {}",
                    names, m.symbol, m.description
                ));
            }

            self.console.print("");
            self.console.print("[bold cyan]LETTERS[/]");
            self.console
                .print(&format!("  [green]{}[/]", letters.join(" ")));

            self.console.print("");
            self.console.print("[bold cyan]NUMBERS[/]");
            self.console
                .print(&format!("  [green]{}[/]", numbers.join(" ")));

            self.console.print("");
            self.console.print("[bold cyan]FUNCTION KEYS[/]");
            self.console
                .print(&format!("  [green]{}[/]", function_keys.join(" ")));

            self.console.print("");
            self.console.print("[bold cyan]SPECIAL KEYS[/]");
            for k in &special {
                self.console
                    .print(&format!("  [green]{}[/]", k.names.join(", ")));
            }

            self.console.print("");
            self.console.print("[bold cyan]ARROW KEYS[/]");
            self.console
                .print(&format!("  [green]{}[/]", arrow_keys.join("  ")));

            self.console.print("");
            self.console.print("[bold cyan]PUNCTUATION[/]");
            for k in &punctuation {
                // Avoid markup interpretation issues with backslash
                let display = k.names.join(", ");
                if display.contains('\\') {
                    // Print without markup for backslash
                    println!("  {}", display);
                } else {
                    self.console.print(&format!("  [green]{}[/]", display));
                }
            }

            self.console.print("");
            self.console.print("[bold cyan]MOUSE[/]");
            self.console
                .print(&format!("  [green]{}[/]", mouse.join("  ")));
            self.console.print(
                "  [dim]Use these names alone. right is the Right Arrow key; right-click is the mouse button.[/]",
            );

            self.console.print("");
            self.console.print("[bold cyan]HOW TO WRITE A MAPPING[/]");
            self.console.print(
                "  [dim]Pedals a, b, or c. Combine modifiers with + (ctrl+a). Sequences are comma-separated (a,b).[/]",
            );
            self.console
                .print("  [dim]A single modifier with no key is allowed (ctrl). Combinations such as shift+alt are not.[/]");
            self.console
                .print("  [dim]clear removes that pedal's mapping and cannot be combined with other keys.[/]");
            self.console.print(
                "  [dim]savant erase --dry-run previews a device-wide wipe of every pedal. Add --yes only to erase.[/]",
            );
            self.console.print(
                "  [dim]No media keys, delays, or repeats. Mouse clicks use the names above, not mouse or click.[/]",
            );
            self.console
                .print("  [dim]F1-F12 work. F13-F24 are a device limitation (the write may succeed, but the pedal will not send those keys).[/]");

            self.console.print("");
            self.console.print("[bold cyan]EXAMPLES[/]");
            self.console
                .print("  [dim]Programming-mode write (preview first):[/]");
            self.console
                .print("  [yellow]savant program --pedal a --action a --dry-run[/]");
            self.console
                .print("  [yellow]savant program --pedal c --action ctrl+a,b --dry-run[/]");
            self.console
                .print("  [yellow]savant program --pedal a --action left-click --dry-run[/]");
            self.console
                .print("  [yellow]savant program --pedal a --action clear --dry-run[/]");
            self.console.print("  [yellow]savant erase --dry-run[/]");
        }

        Ok(())
    }

    /// Refuse preset/profile apply paths. Those commands do not write the pedal.
    pub(crate) fn refuse_unverified_apply(&self, attempted: &str) -> Result<()> {
        let message = format!(
            "{attempted} does not write the pedal. Use savant program --pedal <a|b|c> --action <chord[,chord...]|clear> --dry-run to preview (add --yes only to write)."
        );
        if self.json_output {
            let output = serde_json::json!({
                "error": "unsupported_program_apply",
                "attempted": attempted,
                "message": message,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            self.console
                .print(&format!("[bold red]Error:[/] {}", message));
            self.console.print("");
            self.console.print(
                "  Use [bold yellow]savant program --pedal a --action a --dry-run[/] to preview a mapping.",
            );
        }
        Err(anyhow!(message))
    }

    pub(crate) fn program(
        &self,
        pedal: &str,
        action: &str,
        dry_run: bool,
        yes: bool,
    ) -> Result<()> {
        self.verbose(&format!(
            "Validating programming mapping: pedal='{}' action='{}'",
            pedal, action
        ));
        let planned = prepare_program(pedal, action)?;
        self.verbose(&format!(
            "Encoded pedal {} action {} ({} bytes)",
            planned.pedal,
            planned.action,
            planned.payload.len()
        ));

        let setup = planned.setup;
        let payload_hex = hex::encode(&planned.payload);
        let confirmation = match planned.action {
            ProgramAction::Mouse(_) => {
                "Now switch to Play, unplug, replug, and tap the pedal. Watch the mouse pointer; savant monitor only shows keyboard keys."
            }
            ProgramAction::ModifierOnly(_) => {
                "Now switch to Play, unplug, replug, and tap the pedal. A modifier-only mapping does not type a character by itself."
            }
            _ => {
                "Now switch to Play, unplug, replug, and tap the pedal (for example with savant monitor)."
            }
        };

        let json = |wrote: bool, bytes_written: Option<usize>| JsonProgramOutput {
            dry_run,
            wrote,
            pedal: planned.pedal.to_string().to_ascii_lowercase(),
            action: planned.action.to_string(),
            setup: JsonProgramSetup {
                bm_request_type: format!("0x{:02X}", setup.bm_request_type),
                direction: "Out".to_string(),
                transfer_type: "Vendor".to_string(),
                recipient: "Endpoint".to_string(),
                b_request: setup.b_request,
                w_value: setup.w_value,
                w_index: setup.w_index,
                w_length: planned.payload.len(),
            },
            payload_hex: payload_hex.clone(),
            payload: planned.payload.clone(),
            bytes_written,
            confirmation: if wrote {
                Some(confirmation.to_string())
            } else {
                None
            },
        };

        if dry_run {
            if self.json_output {
                println!("{}", serde_json::to_string_pretty(&json(false, None))?);
                return Ok(());
            }

            self.print_banner();
            self.console
                .print("[bold #f39c12]PREVIEW ONLY[/] [bold white]Nothing was written[/]");
            self.console.print("");
            self.print_program_summary(&planned);
            self.print_program_transfer(&planned);
            self.console.print("");
            self.console
                .print("  Add [bold #f1c40f]--yes[/] to write this mapping.");
            self.console.print("");
            return Ok(());
        }

        if !yes {
            return Err(anyhow!(
                "Refusing to write without --yes. Preview with --dry-run, or pass --yes to write this mapping."
            ));
        }

        self.verbose("Opening the Programming-mode pedal for one write");
        let bytes_written = write_programming_request6(&planned.payload, self.timeout_ms)
            .with_context(|| "Programming write failed")?;
        self.verbose(&format!("bytes_written {bytes_written}"));

        if self.json_output {
            println!(
                "{}",
                serde_json::to_string_pretty(&json(true, Some(bytes_written)))?
            );
            return Ok(());
        }

        self.print_banner();
        self.console.print("[bold #2ecc71]Wrote mapping[/]");
        self.console.print("");
        self.print_program_summary(&planned);
        self.print_program_transfer(&planned);
        self.console.print("");
        self.console.print(&format!("  {}", confirmation));
        self.console.print("");
        Ok(())
    }

    pub(crate) fn erase(&self, dry_run: bool, yes: bool) -> Result<()> {
        self.verbose("Validating device-wide erase");
        let planned = prepare_erase();
        self.verbose(&format!(
            "Encoded erase request {} ({} bytes)",
            planned.setup.b_request,
            planned.payload.len()
        ));

        let setup = planned.setup;
        let payload_hex = hex::encode(&planned.payload);
        let confirmation =
            "All pedal mappings are now blank. Switch to Play, unplug, replug, and tap each pedal.";

        let json = |wrote: bool, bytes_written: Option<usize>| JsonEraseOutput {
            dry_run,
            wrote,
            setup: JsonProgramSetup {
                bm_request_type: format!("0x{:02X}", setup.bm_request_type),
                direction: "Out".to_string(),
                transfer_type: "Vendor".to_string(),
                recipient: "Endpoint".to_string(),
                b_request: setup.b_request,
                w_value: setup.w_value,
                w_index: setup.w_index,
                w_length: planned.payload.len(),
            },
            payload_hex: payload_hex.clone(),
            payload: planned.payload.clone(),
            bytes_written,
            confirmation: if wrote {
                Some(confirmation.to_string())
            } else {
                None
            },
        };

        if dry_run {
            if self.json_output {
                println!("{}", serde_json::to_string_pretty(&json(false, None))?);
                return Ok(());
            }

            self.print_banner();
            self.console
                .print("[bold #f39c12]PREVIEW ONLY[/] [bold white]Nothing was written[/]");
            self.console.print("");
            self.print_erase_summary(&planned);
            self.print_erase_transfer(&planned);
            self.console.print("");
            self.console
                .print("  Add [bold #f1c40f]--yes[/] to erase every pedal mapping.");
            self.console.print("");
            return Ok(());
        }

        if !yes {
            return Err(anyhow!(
                "Refusing to erase without --yes. Preview with --dry-run, or pass --yes to erase every pedal mapping."
            ));
        }

        self.verbose("Opening the Programming-mode pedal for one erase");
        let bytes_written = write_programming_request8(&planned.payload, self.timeout_ms)
            .with_context(|| "Erase write failed")?;
        self.verbose(&format!("bytes_written {bytes_written}"));

        if self.json_output {
            println!(
                "{}",
                serde_json::to_string_pretty(&json(true, Some(bytes_written)))?
            );
            return Ok(());
        }

        self.print_banner();
        self.console.print("[bold #2ecc71]Erased all mappings[/]");
        self.console.print("");
        self.print_erase_summary(&planned);
        self.print_erase_transfer(&planned);
        self.console.print("");
        self.console.print(&format!("  {}", confirmation));
        self.console.print("");
        Ok(())
    }

    fn pedal_owner_label(pedal: Pedal) -> &'static str {
        match pedal {
            Pedal::A => "A (left)",
            Pedal::B => "B (middle)",
            Pedal::C => "C (right)",
        }
    }

    fn print_program_summary(&self, planned: &PreparedProgram) {
        self.console.print(&format!(
            "  Pedal:  {}",
            Self::pedal_owner_label(planned.pedal)
        ));
        self.console.print(&format!("  Action: {}", planned.action));
    }

    fn print_program_transfer(&self, planned: &PreparedProgram) {
        let payload = planned.payload.as_slice();
        let spaced: String = payload
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        self.verbose(&format!("pedal {}", planned.pedal));
        self.verbose(&format!("action {}", planned.action));
        self.verbose(&format!(
            "bmRequestType 0x{:02X} (Out/Vendor/Endpoint)",
            planned.setup.bm_request_type
        ));
        self.verbose(&format!("bRequest {}", planned.setup.b_request));
        self.verbose(&format!("wValue {}", planned.setup.w_value));
        self.verbose(&format!("wIndex {}", planned.setup.w_index));
        self.verbose(&format!("wLength {}", payload.len()));
        self.verbose(&format!("payload {spaced}"));
        self.verbose(&format!("payload_hex {}", hex::encode(payload)));
    }

    fn print_erase_summary(&self, _planned: &PreparedErase) {
        self.console.print("  Scope:  every pedal (A, B, and C)");
        self.console.print("  Action: erase all mappings");
    }

    fn print_erase_transfer(&self, planned: &PreparedErase) {
        let payload = planned.payload.as_slice();
        let spaced: String = payload
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        self.verbose("scope every pedal");
        self.verbose("action erase");
        self.verbose(&format!(
            "bmRequestType 0x{:02X} (Out/Vendor/Endpoint)",
            planned.setup.bm_request_type
        ));
        self.verbose(&format!("bRequest {}", planned.setup.b_request));
        self.verbose(&format!("wValue {}", planned.setup.w_value));
        self.verbose(&format!("wIndex {}", planned.setup.w_index));
        self.verbose(&format!("wLength {}", payload.len()));
        self.verbose(&format!("payload {spaced}"));
        self.verbose(&format!("payload_hex {}", hex::encode(payload)));
    }
}

/// Parse CLI arguments and dispatch to the matching command.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let timeout_ms = cli.timeout.unwrap_or(DEFAULT_USB_TIMEOUT_MS);
    let savant = SavantElite::new(cli.verbose, cli.json, timeout_ms)?;

    if cli.verbose {
        eprintln!("[verbose] Verbose mode enabled");
        eprintln!("[verbose] USB timeout: {}ms", timeout_ms);
    }
    if cli.json {
        savant.verbose("JSON output mode enabled");
    }

    match cli.command {
        Commands::Info => {
            savant.find_device()?;
        }
        Commands::Monitor { duration } => {
            savant.monitor(duration)?;
        }
        Commands::Status => {
            savant.status()?;
        }
        Commands::Program {
            pedal,
            action,
            dry_run,
            yes,
        } => {
            savant.program(&pedal, &action, dry_run, yes)?;
        }
        Commands::Erase { dry_run, yes } => {
            savant.erase(dry_run, yes)?;
        }
        Commands::Keys { json } => {
            savant.list_keys(json)?;
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            generate(shell, &mut cmd, name, &mut std::io::stdout());
        }
        Commands::Preset {
            name,
            list,
            show,
            dry_run,
        } => {
            savant.preset(name.as_deref(), list, show, dry_run)?;
        }
        Commands::Config { command } => {
            savant.config(command)?;
        }
        Commands::Doctor => {
            savant.doctor()?;
        }
    }

    Ok(())
}
