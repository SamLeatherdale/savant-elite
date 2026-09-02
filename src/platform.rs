//! Platform diagnostics (`savant doctor`) and OS-specific guidance.

use crate::cli::{JsonDoctorCheck, JsonDoctorOutput, JsonDoctorSummary, SavantElite};
use crate::config::{profiles_dir, PedalConfig};
use crate::protocol::{KINESIS_VID, PROGRAMMING_PID, SAVANT_ELITE_PID};
use crate::transport::{
    new_hid_api, read_programming_request7, scan_savant_identities, ProgrammingFailureClass,
    ProgrammingStage, ProgrammingTransportError,
};
use anyhow::Result;
use std::fs;

/// Result of the OS compatibility check used by `savant doctor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCheck {
    pub supported: bool,
    pub message: String,
    pub details: Option<String>,
    pub suggestions: Vec<String>,
}

/// Classify platform support for detection and read-only diagnostics.
pub fn platform_check(os: &str) -> PlatformCheck {
    match os {
        "macos" => PlatformCheck {
            supported: true,
            message: "macOS detected (supported)".to_string(),
            details: None,
            suggestions: vec![],
        },
        "windows" => PlatformCheck {
            supported: true,
            message: "Windows detected (supported for detection and read-only diagnostics)"
                .to_string(),
            details: Some(
                "Bind only Programming PID 05F3:0232 to WinUSB. Never bind Play PID 05F3:030C."
                    .to_string(),
            ),
            suggestions: vec![
                "Bind only Programming PID 05F3:0232 to WinUSB".to_string(),
                "Never bind Play PID 05F3:030C to WinUSB; leave it on the HID driver".to_string(),
            ],
        },
        other => PlatformCheck {
            supported: false,
            message: format!("{} detected (unsupported)", other),
            details: None,
            suggestions: vec![
                "savant-elite supports macOS and Windows for detection and read-only diagnostics"
                    .to_string(),
            ],
        },
    }
}

impl SavantElite {
    pub(crate) fn doctor(&self) -> Result<()> {
        self.verbose("Running system diagnostics");

        // Collect all check results
        let mut checks: Vec<JsonDoctorCheck> = Vec::new();

        // Get system info
        let version = env!("CARGO_PKG_VERSION").to_string();
        let platform = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();

        if !self.json_output {
            self.print_banner();

            self.console.print(
                "[bold #3498db]┌─────────────────────────────────────────────────────────────────┐[/]",
            );
            self.console.print(
                "[bold #3498db]│[/]  [bold white]SYSTEM DIAGNOSTICS[/]                                          [bold #3498db]│[/]",
            );
            self.console.print(
                "[bold #3498db]└─────────────────────────────────────────────────────────────────┘[/]",
            );
            self.console.print("");
        }

        // Check 1: Binary/System info
        checks.push(self.doctor_check_binary(&version, &platform, &arch));

        // Check 2: Platform compatibility
        checks.push(self.doctor_check_platform(&platform));

        // Check 3: Device detection
        checks.push(self.doctor_check_device());

        // Check 4: Config file
        checks.push(self.doctor_check_config());

        // Check 5: Profiles directory
        checks.push(self.doctor_check_profiles());

        // Check 6: Input Monitoring (attempt to detect)
        checks.push(self.doctor_check_input_monitoring());

        // Calculate summary
        let passed = checks.iter().filter(|c| c.status == "pass").count();
        let warnings = checks.iter().filter(|c| c.status == "warn").count();
        let failed = checks.iter().filter(|c| c.status == "fail").count();
        let total = checks.len();
        let healthy = failed == 0;

        if self.json_output {
            let output = JsonDoctorOutput {
                version,
                platform,
                arch,
                checks,
                summary: JsonDoctorSummary {
                    total,
                    passed,
                    warnings,
                    failed,
                    healthy,
                },
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            // Print summary
            self.console.print("");
            self.console.print(
                "[bold #9b59b6]┌─────────────────────────────────────────────────────────────────┐[/]",
            );
            self.console.print(
                "[bold #9b59b6]│[/]  [bold white]SUMMARY[/]                                                       [bold #9b59b6]│[/]",
            );
            self.console.print(
                "[bold #9b59b6]└─────────────────────────────────────────────────────────────────┘[/]",
            );
            self.console.print("");

            if healthy {
                self.console.print(&format!(
                    "  [bold #2ecc71]✓[/] All {} checks passed{}",
                    total,
                    if warnings > 0 {
                        format!(
                            " ({} warning{})",
                            warnings,
                            if warnings == 1 { "" } else { "s" }
                        )
                    } else {
                        String::new()
                    }
                ));
                self.console.print("");
                self.console
                    .print("  [bold #2ecc71]Your system is ready to use savant-elite![/]");
            } else {
                self.console.print(&format!(
                    "  [bold #e74c3c]✗[/] {} issue{} found ({} passed, {} warning{}, {} failed)",
                    failed + warnings,
                    if failed + warnings == 1 { "" } else { "s" },
                    passed,
                    warnings,
                    if warnings == 1 { "" } else { "s" },
                    failed
                ));
                self.console.print("");
                self.console
                    .print("  [dim]Fix the issues above and run 'savant doctor' again.[/]");
            }
        }

        Ok(())
    }

    fn doctor_check_binary(&self, version: &str, platform: &str, arch: &str) -> JsonDoctorCheck {
        if !self.json_output {
            self.console.print("[bold cyan]Binary:[/]");
            self.console.print(&format!(
                "  [bold #2ecc71]✓[/] savant-elite version {}",
                version
            ));
            self.console
                .print(&format!("  [bold #2ecc71]✓[/] {} ({})", platform, arch));
            self.console.print("");
        }

        JsonDoctorCheck {
            name: "binary".to_string(),
            status: "pass".to_string(),
            message: format!("savant-elite v{} on {} ({})", version, platform, arch),
            details: None,
            suggestions: vec![],
        }
    }

    fn doctor_check_platform(&self, platform: &str) -> JsonDoctorCheck {
        let check = platform_check(platform);

        if !self.json_output {
            self.console.print("[bold cyan]Platform:[/]");
            if check.supported {
                self.console
                    .print(&format!("  [bold #2ecc71]✓[/] {}", check.message));
                for suggestion in &check.suggestions {
                    self.console.print(&format!("    [dim]→ {}[/]", suggestion));
                }
            } else {
                self.console
                    .print(&format!("  [bold #e74c3c]✗[/] {}", check.message));
                for suggestion in &check.suggestions {
                    self.console.print(&format!("    [dim]→ {}[/]", suggestion));
                }
            }
            self.console.print("");
        }

        JsonDoctorCheck {
            name: "platform".to_string(),
            status: if check.supported {
                "pass".to_string()
            } else {
                "fail".to_string()
            },
            message: check.message,
            details: check.details,
            suggestions: check.suggestions,
        }
    }

    fn doctor_check_device(&self) -> JsonDoctorCheck {
        if !self.json_output {
            self.console.print("[bold cyan]Device:[/]");
        }

        let identities = match scan_savant_identities() {
            Ok(identities) => identities,
            Err(error) => return self.doctor_device_transport_failure(error),
        };

        if identities.programming {
            return self.doctor_device_programming();
        }

        if identities.play {
            if !self.json_output {
                self.console
                    .print("  [bold #2ecc71]✓[/] Savant Elite detected");
                self.console
                    .print("  [bold #f39c12]⚠[/] Mode: [bold #2ecc71]Play[/]");
                self.console
                    .print("    [dim]→ To program: flip switch to Program, replug USB[/]");
                self.console
                    .print("    [dim]→ To monitor: this mode is correct[/]");
                self.console.print(
                    "    [dim]→ Never bind Play PID 05F3:030C to WinUSB; leave it on HID[/]",
                );
                self.console.print("");
            }
            return JsonDoctorCheck {
                name: "device".to_string(),
                status: "warn".to_string(),
                message: "Savant Elite detected in PLAY mode".to_string(),
                details: Some(format!(
                    "VID=0x{:04X}, PID=0x{:04X}",
                    KINESIS_VID, SAVANT_ELITE_PID
                )),
                suggestions: vec![
                    "To program: flip switch to Program, replug USB".to_string(),
                    "To monitor: this mode is correct".to_string(),
                    "Never bind Play PID 05F3:030C to WinUSB; leave it on the HID driver"
                        .to_string(),
                ],
            };
        }

        if !self.json_output {
            self.console
                .print("  [bold #e74c3c]✗[/] No Savant Elite device found");
            self.console
                .print("    [dim]→ Connect the device via USB[/]");
            self.console.print("    [dim]→ Try a different USB port[/]");
            self.console
                .print("    [dim]→ Check if another app has the device open[/]");
            self.console.print("");
        }
        JsonDoctorCheck {
            name: "device".to_string(),
            status: "fail".to_string(),
            message: "No Savant Elite device found".to_string(),
            details: None,
            suggestions: vec![
                "Connect the device via USB".to_string(),
                "Try a different USB port".to_string(),
                "Check if another app has the device open".to_string(),
            ],
        }
    }

    fn doctor_device_programming(&self) -> JsonDoctorCheck {
        match read_programming_request7(self.timeout_ms) {
            Ok(bytes) => {
                let hex = bytes
                    .iter()
                    .map(|byte| format!("{:02X}", byte))
                    .collect::<Vec<_>>()
                    .join(" ");
                if !self.json_output {
                    self.console
                        .print("  [bold #2ecc71]✓[/] Savant Elite detected");
                    self.console.print(
                        "  [bold #2ecc71]✓[/] Mode: [bold #f39c12]Program[/]  Ready to program",
                    );
                    self.verbose(&format!(
                        "Programming PID 0x{:04X}; request-7 raw bytes ({}){}",
                        PROGRAMMING_PID,
                        bytes.len(),
                        if hex.is_empty() {
                            String::new()
                        } else {
                            format!(": {hex}")
                        }
                    ));
                    self.console.print("");
                }
                JsonDoctorCheck {
                    name: "device".to_string(),
                    status: "pass".to_string(),
                    message: "Savant Elite detected in PROGRAMMING mode".to_string(),
                    details: Some(format!(
                        "VID=0x{:04X}, PID=0x{:04X}; request-7 raw bytes ({}){}",
                        KINESIS_VID,
                        PROGRAMMING_PID,
                        bytes.len(),
                        if hex.is_empty() {
                            String::new()
                        } else {
                            format!(": {}", hex)
                        }
                    )),
                    suggestions: vec![],
                }
            }
            Err(error) => self.doctor_device_transport_failure(error),
        }
    }

    fn doctor_device_transport_failure(&self, error: ProgrammingTransportError) -> JsonDoctorCheck {
        let suggestions = error.suggestions();
        let message = device_failure_message(&error);

        if !self.json_output {
            if error.stage == ProgrammingStage::Enumerate {
                self.console
                    .print(&format!("  [bold #e74c3c]✗[/] {}", message));
            } else if error.class == ProgrammingFailureClass::NotFound {
                self.console
                    .print("  [bold #e74c3c]✗[/] No Savant Elite device found");
            } else {
                self.console
                    .print("  [bold #2ecc71]✓[/] Savant Elite detected");
                self.console
                    .print(&format!("  [bold #e74c3c]✗[/] {}", message));
            }
            for suggestion in &suggestions {
                self.console.print(&format!("    [dim]→ {}[/]", suggestion));
            }
            self.console.print("");
        }

        JsonDoctorCheck {
            name: "device".to_string(),
            status: "fail".to_string(),
            message,
            details: if error.source.is_empty() {
                None
            } else {
                Some(error.source)
            },
            suggestions,
        }
    }

    fn doctor_check_config(&self) -> JsonDoctorCheck {
        if !self.json_output {
            self.console.print("[bold cyan]Config:[/]");
        }

        let path = PedalConfig::config_path();

        if path.exists() {
            match PedalConfig::load() {
                Some(config) => {
                    if !self.json_output {
                        self.console
                            .print("  [bold #2ecc71]✓[/] Config file exists and is valid");
                        self.console
                            .print(&format!("    [dim]Path: {}[/]", path.display()));
                        self.console.print(&format!(
                            "    [dim]Config: left={}, middle={}, right={}[/]",
                            config.left, config.middle, config.right
                        ));
                        self.console.print("");
                    }
                    JsonDoctorCheck {
                        name: "config".to_string(),
                        status: "pass".to_string(),
                        message: "Config file exists and is valid".to_string(),
                        details: Some(format!(
                            "left={}, middle={}, right={}",
                            config.left, config.middle, config.right
                        )),
                        suggestions: vec![],
                    }
                }
                None => {
                    if !self.json_output {
                        self.console
                            .print("  [bold #f39c12]⚠[/] Config file exists but is invalid");
                        self.console
                            .print(&format!("    [dim]Path: {}[/]", path.display()));
                        self.console
                            .print("    [dim]→ Check file format (left=X, middle=Y, right=Z)[/]");
                        self.console.print("");
                    }
                    JsonDoctorCheck {
                        name: "config".to_string(),
                        status: "warn".to_string(),
                        message: "Config file exists but is invalid".to_string(),
                        details: Some(path.display().to_string()),
                        suggestions: vec![
                            "Check file format (left=X, middle=Y, right=Z)".to_string()
                        ],
                    }
                }
            }
        } else {
            if !self.json_output {
                self.console
                    .print("  [bold #f39c12]⚠[/] Config file not found (OK for first-time use)");
                self.console
                    .print(&format!("    [dim]Path: {}[/]", path.display()));
                self.console
                    .print("    [dim]→ Run 'savant program' to create a configuration[/]");
                self.console.print("");
            }
            JsonDoctorCheck {
                name: "config".to_string(),
                status: "warn".to_string(),
                message: "Config file not found (OK for first-time use)".to_string(),
                details: Some(path.display().to_string()),
                suggestions: vec!["Run 'savant program' to create a configuration".to_string()],
            }
        }
    }

    fn doctor_check_profiles(&self) -> JsonDoctorCheck {
        if !self.json_output {
            self.console.print("[bold cyan]Profiles:[/]");
        }

        let dir = profiles_dir();

        if dir.exists() {
            // Count profiles
            let count = fs::read_dir(&dir)
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().is_some_and(|ext| ext == "conf"))
                        .count()
                })
                .unwrap_or(0);

            if !self.json_output {
                self.console.print(&format!(
                    "  [bold #2ecc71]✓[/] Profiles directory exists ({} profile{})",
                    count,
                    if count == 1 { "" } else { "s" }
                ));
                self.console
                    .print(&format!("    [dim]Path: {}[/]", dir.display()));
                self.console.print("");
            }
            JsonDoctorCheck {
                name: "profiles".to_string(),
                status: "pass".to_string(),
                message: format!(
                    "Profiles directory exists ({} profile{})",
                    count,
                    if count == 1 { "" } else { "s" }
                ),
                details: Some(dir.display().to_string()),
                suggestions: vec![],
            }
        } else {
            if !self.json_output {
                self.console.print(
                    "  [bold #f39c12]⚠[/] Profiles directory not found (OK for first-time use)",
                );
                self.console
                    .print(&format!("    [dim]Path: {}[/]", dir.display()));
                self.console
                    .print("    [dim]→ Run 'savant config save <name>' to create a profile[/]");
                self.console.print("");
            }
            JsonDoctorCheck {
                name: "profiles".to_string(),
                status: "warn".to_string(),
                message: "Profiles directory not found (OK for first-time use)".to_string(),
                details: Some(dir.display().to_string()),
                suggestions: vec!["Run 'savant config save <name>' to create a profile".to_string()],
            }
        }
    }

    fn doctor_check_input_monitoring(&self) -> JsonDoctorCheck {
        if !self.json_output {
            self.console.print("[bold cyan]Permissions:[/]");
        }

        // Try to initialize HID API and open a device to check permissions
        // This is a heuristic - if we can enumerate HID devices, permissions are likely OK
        let hid_api = match new_hid_api() {
            Ok(api) => api,
            Err(e) => {
                let err_str = e.to_string().to_lowercase();
                let is_permission_error = err_str.contains("privilege")
                    || err_str.contains("permission")
                    || err_str.contains("access");

                if is_permission_error {
                    if !self.json_output {
                        self.console.print(
                            "  [bold #e74c3c]✗[/] Input Monitoring permission may be required",
                        );
                        self.console.print("    [dim]→ Open System Settings → Privacy & Security → Input Monitoring[/]");
                        self.console
                            .print("    [dim]→ Add your terminal app (Terminal, iTerm2, etc.)[/]");
                        self.console.print("    [dim]→ Restart your terminal[/]");
                        self.console.print("");
                    }
                    return JsonDoctorCheck {
                        name: "permissions".to_string(),
                        status: "fail".to_string(),
                        message: "Input Monitoring permission may be required".to_string(),
                        details: Some(e.to_string()),
                        suggestions: vec![
                            "Open System Settings → Privacy & Security → Input Monitoring"
                                .to_string(),
                            "Add your terminal app (Terminal, iTerm2, etc.)".to_string(),
                            "Restart your terminal".to_string(),
                        ],
                    };
                }

                if !self.json_output {
                    self.console.print(&format!(
                        "  [bold #f39c12]⚠[/] Could not initialize HID API: {}",
                        e
                    ));
                    self.console.print("");
                }
                return JsonDoctorCheck {
                    name: "permissions".to_string(),
                    status: "warn".to_string(),
                    message: format!("Could not initialize HID API: {}", e),
                    details: None,
                    suggestions: vec![],
                };
            }
        };

        // Check if we can enumerate devices with our VID
        let devices: Vec<_> = hid_api
            .device_list()
            .filter(|d| d.vendor_id() == KINESIS_VID)
            .collect();

        if !devices.is_empty() {
            let message = if std::env::consts::OS == "windows" {
                "HID API accessible (Play-mode HID enumeration available)".to_string()
            } else {
                "HID API accessible (Input Monitoring OK)".to_string()
            };
            if !self.json_output {
                self.console
                    .print(&format!("  [bold #2ecc71]✓[/] {}", message));
                self.console.print("");
            }
            JsonDoctorCheck {
                name: "permissions".to_string(),
                status: "pass".to_string(),
                message,
                details: None,
                suggestions: vec![],
            }
        } else {
            // Device not found via HID, but we already checked via libusb
            // This might just mean device is in program mode (not HID)
            if !self.json_output {
                self.console
                    .print("  [bold #2ecc71]✓[/] HID API initialized successfully");
                self.console
                    .print("    [dim]Note: Device monitoring requires device in PLAY mode[/]");
                self.console.print("");
            }
            JsonDoctorCheck {
                name: "permissions".to_string(),
                status: "pass".to_string(),
                message: "HID API initialized successfully".to_string(),
                details: Some("Device monitoring requires device in PLAY mode".to_string()),
                suggestions: vec![],
            }
        }
    }
}

/// Human-readable doctor message for a classified Programming-mode failure.
pub fn device_failure_message(error: &ProgrammingTransportError) -> String {
    match (error.stage, error.class) {
        (_, ProgrammingFailureClass::NotFound) => "No Savant Elite device found".to_string(),
        (ProgrammingStage::Enumerate, _) => error.message(),
        (ProgrammingStage::Open, ProgrammingFailureClass::Access) => {
            "Savant Elite detected in PROGRAMMING mode but open failed (access denied)".to_string()
        }
        (ProgrammingStage::Open, ProgrammingFailureClass::DriverBinding) => {
            "Savant Elite detected in PROGRAMMING mode but open failed (driver binding)".to_string()
        }
        (ProgrammingStage::Open, _) => {
            "Savant Elite detected in PROGRAMMING mode but could not be opened".to_string()
        }
        (ProgrammingStage::Claim, ProgrammingFailureClass::Access) => {
            "Savant Elite detected in PROGRAMMING mode but claim failed (access denied)".to_string()
        }
        (ProgrammingStage::Claim, ProgrammingFailureClass::Busy) => {
            "Savant Elite detected in PROGRAMMING mode but claim failed (busy)".to_string()
        }
        (ProgrammingStage::Claim, ProgrammingFailureClass::DriverBinding) => {
            "Savant Elite detected in PROGRAMMING mode but claim failed (driver binding)"
                .to_string()
        }
        (ProgrammingStage::Claim, ProgrammingFailureClass::KernelDriver) => {
            "Savant Elite detected in PROGRAMMING mode but kernel-driver detach failed".to_string()
        }
        (ProgrammingStage::Claim, _) => {
            "Savant Elite detected in PROGRAMMING mode but could not claim interface 0".to_string()
        }
        (ProgrammingStage::Read, _) => {
            "Savant Elite detected in PROGRAMMING mode but could not confirm it is ready"
                .to_string()
        }
        (ProgrammingStage::Write, _) => {
            "Savant Elite detected in PROGRAMMING mode but could not write the mapping".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_is_supported_for_readonly_diagnostics() {
        let check = platform_check("windows");
        assert!(check.supported);
        assert!(check.message.contains("Windows detected"));
        assert!(check.message.contains("supported"));
        assert!(!check.message.contains("unsupported"));
        let details = check
            .details
            .expect("Windows should include WinUSB guidance");
        assert!(details.contains("05F3:0232"));
        assert!(details.contains("05F3:030C"));
        assert!(details.contains("Never"));
        assert!(check
            .suggestions
            .iter()
            .any(|tip| tip.contains("05F3:0232") && tip.contains("WinUSB")));
        assert!(check
            .suggestions
            .iter()
            .any(|tip| tip.contains("Never bind Play PID 05F3:030C")));
    }

    #[test]
    fn macos_remains_supported() {
        let check = platform_check("macos");
        assert!(check.supported);
        assert_eq!(check.message, "macOS detected (supported)");
        assert!(check.suggestions.is_empty());
    }

    #[test]
    fn linux_remains_unsupported() {
        let check = platform_check("linux");
        assert!(!check.supported);
        assert!(check.message.contains("linux detected (unsupported)"));
    }

    #[test]
    fn device_failure_messages_distinguish_no_device_from_access_and_driver() {
        let missing = ProgrammingTransportError::not_found();
        let access =
            ProgrammingTransportError::from_rusb(ProgrammingStage::Open, rusb::Error::Access);
        let driver =
            ProgrammingTransportError::from_rusb(ProgrammingStage::Open, rusb::Error::NotSupported);
        let claim =
            ProgrammingTransportError::from_rusb(ProgrammingStage::Claim, rusb::Error::Busy);

        assert_eq!(
            device_failure_message(&missing),
            "No Savant Elite device found"
        );
        assert!(device_failure_message(&access).contains("open failed (access denied)"));
        assert!(device_failure_message(&driver).contains("open failed (driver binding)"));
        assert!(device_failure_message(&claim).contains("claim failed (busy)"));
        assert_ne!(
            device_failure_message(&missing),
            device_failure_message(&access)
        );
        assert_ne!(
            device_failure_message(&access),
            device_failure_message(&driver)
        );
    }
}
