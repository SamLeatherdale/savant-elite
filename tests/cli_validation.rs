//! E2E integration tests for CLI argument validation
//!
//! These tests verify that the CLI correctly rejects invalid inputs
//! before attempting any device operations.

use assert_cmd::Command;
use predicates::prelude::*;

/// Helper to get the savant command
fn savant() -> Command {
    Command::new(env!("CARGO_BIN_EXE_savant"))
}

// ============================================================================
// Help and Version Tests
// ============================================================================

#[test]
fn cli_shows_help() {
    savant()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Kinesis Savant Elite"))
        .stdout(predicate::str::contains("program"))
        .stdout(predicate::str::contains("monitor"))
        .stdout(predicate::str::contains("info"))
        .stdout(predicate::str::contains("probe").not())
        .stdout(predicate::str::contains("raw-cmd").not());
}

#[test]
fn cli_shows_version() {
    savant()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("savant"));
}

#[test]
fn cli_shows_subcommand_help() {
    savant()
        .args(["program", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--pedal"))
        .stdout(predicate::str::contains("--action"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--yes"))
        .stdout(predicate::str::contains("--left").not())
        .stdout(predicate::str::contains("--middle").not())
        .stdout(predicate::str::contains("--right").not());
}

// ============================================================================
// Verified program path (dry-run, no device required)
// ============================================================================

const VERIFIED_MAPPINGS: [(&str, &str, &str); 6] = [
    ("a", "a", "01 00 00 01 02 04 fe 04"),
    ("a", "b", "01 00 00 01 02 05 fe 05"),
    ("b", "a", "02 00 00 01 02 04 fe 04"),
    ("a", "ctrl+a", "01 00 00 02 04 f0 04 fe 04 fe f0"),
    ("a", "a,b", "01 00 00 06 00 04 fe 04 05 fe 05"),
    ("a", "ctrl+a,b", "01 00 00 09 00 f0 04 fe 04 fe f0 05 fe 05"),
];

#[test]
fn cli_program_requires_pedal_and_action() {
    savant()
        .args(["program", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--pedal"));
}

#[test]
fn cli_program_rejects_obsolete_left_middle_right() {
    savant()
        .args(["program", "--left", "a", "--dry-run"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument").or(predicate::str::contains("--left")),
        );
}

#[test]
fn cli_program_dry_run_all_verified_mappings() {
    for (pedal, action, payload) in VERIFIED_MAPPINGS {
        savant()
            .env("SAVANT_FAIL_ON_USB", "1")
            .args([
                "-v",
                "program",
                "--pedal",
                pedal,
                "--action",
                action,
                "--dry-run",
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("PREVIEW ONLY"))
            .stdout(predicate::str::contains("Nothing was written"))
            .stdout(predicate::str::contains("bmRequestType").not())
            .stdout(predicate::str::contains("bytes_written").not())
            .stderr(predicate::str::contains("bmRequestType"))
            .stderr(predicate::str::contains("0x42"))
            .stderr(predicate::str::contains("Out/Vendor/Endpoint"))
            .stderr(predicate::str::contains("bRequest"))
            .stderr(predicate::str::contains("wValue"))
            .stderr(predicate::str::contains("wIndex"))
            .stderr(predicate::str::contains(payload));
    }
}

#[test]
fn cli_program_dry_run_json_is_machine_checkable() {
    let output = savant()
        .env("SAVANT_FAIL_ON_USB", "1")
        .args([
            "--json",
            "program",
            "--pedal",
            "a",
            "--action",
            "a",
            "--dry-run",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("program --json --dry-run should be valid JSON");
    assert_eq!(json.get("dry_run").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(json.get("wrote").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(json.get("pedal").and_then(|v| v.as_str()), Some("a"));
    assert_eq!(json.get("action").and_then(|v| v.as_str()), Some("a"));
    assert_eq!(
        json.get("payload_hex").and_then(|v| v.as_str()),
        Some("010000010204fe04")
    );
    let setup = json.get("setup").expect("setup object");
    assert_eq!(
        setup.get("bmRequestType").and_then(|v| v.as_str()),
        Some("0x42")
    );
    assert_eq!(setup.get("direction").and_then(|v| v.as_str()), Some("Out"));
    assert_eq!(setup.get("type").and_then(|v| v.as_str()), Some("Vendor"));
    assert_eq!(
        setup.get("recipient").and_then(|v| v.as_str()),
        Some("Endpoint")
    );
    assert_eq!(setup.get("bRequest").and_then(|v| v.as_u64()), Some(6));
    assert_eq!(setup.get("wValue").and_then(|v| v.as_u64()), Some(0));
    assert_eq!(setup.get("wIndex").and_then(|v| v.as_u64()), Some(0));
    assert_eq!(setup.get("wLength").and_then(|v| v.as_u64()), Some(8));
}

#[test]
fn cli_program_dry_run_does_not_enumerate_usb() {
    savant()
        .env("SAVANT_FAIL_ON_USB", "1")
        .args(["program", "--pedal", "a", "--action", "a", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing was written"));
}

#[test]
fn cli_program_missing_yes_refuses_write_without_usb() {
    savant()
        .env("SAVANT_FAIL_ON_USB", "1")
        .args(["program", "--pedal", "a", "--action", "a"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Refusing to write without --yes"));
}

#[test]
fn cli_program_dry_run_extended_mappings() {
    let rows = [
        ("c", "a", "03 00 00 01 02 04 fe 04"),
        ("a", "clear", "01 00 00 00 00"),
        ("a", "a,b,c", "01 00 00 09 00 04 fe 04 05 fe 05 06 fe 06"),
        (
            "a",
            "shift+a,b",
            "01 00 00 09 00 f1 04 fe 04 fe f1 05 fe 05",
        ),
        ("a", "f1", "01 00 00 01 02 3a fe 3a"),
        ("a", "f12", "01 00 00 01 02 45 fe 45"),
        ("a", "pause", "01 00 00 01 02 48 fe 48"),
        ("a", "rctrl+a", "01 00 00 02 04 f4 04 fe 04 fe f4"),
        ("b", "b", "02 00 00 01 02 05 fe 05"),
    ];
    for (pedal, action, payload) in rows {
        savant()
            .env("SAVANT_FAIL_ON_USB", "1")
            .args([
                "-v",
                "program",
                "--pedal",
                pedal,
                "--action",
                action,
                "--dry-run",
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("PREVIEW ONLY"))
            .stdout(predicate::str::contains(payload).not())
            .stderr(predicate::str::contains(payload));
    }
}

#[test]
fn cli_program_rejects_malformed_and_consumer_mappings() {
    savant()
        .env("SAVANT_FAIL_ON_USB", "1")
        .args(["program", "--pedal", "a", "--action", "play", "--dry-run"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("consumer")
                .or(predicate::str::contains("media"))
                .or(predicate::str::contains("out of scope")),
        );

    savant()
        .env("SAVANT_FAIL_ON_USB", "1")
        .args(["program", "--pedal", "a", "--action", "mute", "--dry-run"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("consumer")
                .or(predicate::str::contains("media"))
                .or(predicate::str::contains("out of scope")),
        );

    savant()
        .env("SAVANT_FAIL_ON_USB", "1")
        .args([
            "program",
            "--pedal",
            "a",
            "--action",
            "clear,a",
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("clear"));

    savant()
        .env("SAVANT_FAIL_ON_USB", "1")
        .args(["program", "--pedal", "d", "--action", "a", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported pedal"));
}

#[test]
fn cli_program_rejects_f13_f24_before_usb() {
    for action in ["f13", "f24", "F24", "rctrl+f24", "a,f13"] {
        savant()
            .env("SAVANT_FAIL_ON_USB", "1")
            .args(["program", "--pedal", "a", "--action", action, "--dry-run"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Device limitation"))
            .stderr(predicate::str::contains("F13-F24"))
            .stderr(predicate::str::contains("no Play event"));
    }
}

// ============================================================================
// Removed reverse-engineering commands
// ============================================================================

#[test]
fn cli_probe_is_removed() {
    savant().arg("probe").assert().failure().stderr(
        predicate::str::contains("unrecognized subcommand")
            .or(predicate::str::contains("invalid subcommand")),
    );
}

#[test]
fn cli_raw_cmd_is_removed() {
    savant()
        .args(["raw-cmd", "--cmd", "b5"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unrecognized subcommand")
                .or(predicate::str::contains("invalid subcommand")),
        );
}

// ============================================================================
// Monitor Command Tests
// ============================================================================

#[test]
fn cli_monitor_accepts_valid_duration() {
    // This will fail because no device, but the argument parsing should succeed
    // We check that it doesn't fail with a parsing error
    let result = savant().args(["monitor", "--duration", "10"]).assert();

    // Should fail due to device issues, not argument parsing
    let output = result.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("invalid value"),
        "Should accept valid duration"
    );
}

#[test]
fn cli_monitor_rejects_negative_duration() {
    // Clap treats "-1" as an unknown flag, so it says "unexpected argument"
    savant()
        .args(["monitor", "--duration", "-1"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("unexpected argument")
                .or(predicate::str::contains("invalid value")),
        );
}

// ============================================================================
// Unknown Subcommand Tests
// ============================================================================

#[test]
fn cli_rejects_unknown_subcommand() {
    savant().arg("unknown").assert().failure().stderr(
        predicate::str::contains("unrecognized subcommand")
            .or(predicate::str::contains("invalid subcommand")),
    );
}

#[test]
fn cli_requires_subcommand() {
    savant()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage").or(predicate::str::contains("subcommand")));
}

// ============================================================================
// Keys Command Tests
// ============================================================================

#[test]
fn cli_keys_shows_modifiers() {
    savant()
        .arg("keys")
        .assert()
        .success()
        .stdout(predicate::str::contains("MODIFIERS"))
        .stdout(predicate::str::contains("cmd"))
        .stdout(predicate::str::contains("ctrl"))
        .stdout(predicate::str::contains("shift"))
        .stdout(predicate::str::contains("alt"));
}

#[test]
fn cli_keys_shows_all_categories() {
    savant()
        .arg("keys")
        .assert()
        .success()
        .stdout(predicate::str::contains("LETTERS"))
        .stdout(predicate::str::contains("NUMBERS"))
        .stdout(predicate::str::contains("FUNCTION KEYS"))
        .stdout(predicate::str::contains("f12"))
        .stdout(predicate::str::contains("f13").not())
        .stdout(predicate::str::contains("f24").not())
        .stdout(predicate::str::contains("SPECIAL KEYS"))
        .stdout(predicate::str::contains("ARROW KEYS"))
        .stdout(predicate::str::contains("PUNCTUATION"))
        .stdout(predicate::str::contains("EXAMPLES"));
}

#[test]
fn cli_keys_json_is_valid() {
    let output = savant()
        .args(["keys", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    // Verify it's valid JSON by parsing it
    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("keys --json should produce valid JSON");

    // Verify structure
    assert!(
        json.get("modifiers").is_some(),
        "JSON should have modifiers"
    );
    assert!(json.get("keys").is_some(), "JSON should have keys");
}

#[test]
fn cli_keys_help() {
    savant()
        .args(["keys", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--json"))
        .stdout(predicate::str::contains("List all valid key names"));
}

// ============================================================================
// Completions Command Tests
// ============================================================================

#[test]
fn cli_completions_zsh() {
    savant()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#compdef savant"))
        .stdout(predicate::str::contains("_savant"));
}

#[test]
fn cli_completions_bash() {
    savant()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_savant()"))
        .stdout(predicate::str::contains("COMPREPLY"));
}

#[test]
fn cli_completions_fish() {
    savant()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("__fish_savant"))
        .stdout(predicate::str::contains("complete -c savant"));
}

#[test]
fn cli_completions_help() {
    savant()
        .args(["completions", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Generate shell completion scripts",
        ))
        .stdout(predicate::str::contains("bash"))
        .stdout(predicate::str::contains("zsh"))
        .stdout(predicate::str::contains("fish"));
}

#[test]
fn cli_completions_rejects_invalid_shell() {
    savant()
        .args(["completions", "invalid"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

// ============================================================================
// Verbose Mode Tests
// ============================================================================

#[test]
fn cli_verbose_flag_accepted() {
    // -v flag should be accepted on any command
    savant().args(["-v", "keys"]).assert().success();
}

#[test]
fn cli_verbose_long_flag_accepted() {
    // --verbose flag should be accepted on any command
    savant().args(["--verbose", "keys"]).assert().success();
}

#[test]
fn cli_verbose_output_goes_to_stderr() {
    // Verbose output should go to stderr, not stdout
    savant()
        .args(["-v", "keys", "--json"])
        .assert()
        .success()
        .stderr(predicate::str::contains("[verbose]"))
        .stdout(predicate::str::contains("[verbose]").not());
}

#[test]
fn cli_verbose_shows_mode_enabled() {
    savant()
        .args(["-v", "keys"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Verbose mode enabled"));
}

#[test]
fn cli_verbose_with_dry_run() {
    savant()
        .env("SAVANT_FAIL_ON_USB", "1")
        .args([
            "-v",
            "program",
            "--pedal",
            "a",
            "--action",
            "a",
            "--dry-run",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Validating programming mapping"));
}

// ============================================================================
// JSON Output Tests
// ============================================================================

#[test]
fn cli_json_flag_accepted() {
    // --json flag should be accepted on info command
    // This will fail because no device, but we check JSON output structure
    let result = savant().args(["--json", "info"]).assert();
    let output = result.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should output valid JSON (even if device not found)
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("info --json should produce valid JSON");
    assert!(
        json.get("device").is_some(),
        "JSON should have device field"
    );
}

#[test]
fn cli_json_info_has_correct_structure() {
    let output = savant()
        .args(["--json", "info"])
        .assert()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("info --json should produce valid JSON");

    // Check device structure
    let device = json.get("device").expect("should have device field");
    assert!(
        device.get("detected").is_some(),
        "device should have detected field"
    );
    assert!(device.get("vid").is_some(), "device should have vid field");
    assert!(
        device.get("interfaces").is_some(),
        "device should have interfaces field"
    );
}

#[test]
fn cli_json_status_produces_valid_json() {
    let result = savant().args(["--json", "status"]).assert();
    let output = result.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("status --json should produce valid JSON");
    assert!(
        json.get("detected").is_some(),
        "JSON should have detected field"
    );
    assert!(
        json.get("ready_to_program").is_some(),
        "JSON should have ready_to_program field"
    );
    assert!(
        json.get("devices").is_some(),
        "JSON should have devices field"
    );
}

#[test]
fn cli_json_output_goes_to_stdout() {
    // JSON output should go to stdout, not stderr
    savant()
        .args(["--json", "info"])
        .assert()
        .stdout(predicate::str::contains("\"device\""))
        .stderr(predicate::str::contains("\"device\"").not());
}

#[test]
fn cli_json_with_verbose() {
    // JSON and verbose should work together
    savant()
        .args(["--json", "-v", "info"])
        .assert()
        .stdout(predicate::str::contains("\"device\""))
        .stderr(predicate::str::contains("[verbose]"));
}

// ============================================================================
// Preset Command Tests
// ============================================================================

#[test]
fn cli_preset_list_shows_all_presets() {
    savant()
        .args(["preset", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("AVAILABLE PRESETS"))
        .stdout(predicate::str::contains("copy-paste"))
        .stdout(predicate::str::contains("undo-redo"))
        .stdout(predicate::str::contains("browser"))
        .stdout(predicate::str::contains("zoom"))
        .stdout(predicate::str::contains("Apply the copy-paste").not())
        .stdout(predicate::str::contains("preset browser --dry-run").not())
        .stdout(predicate::str::contains("savant program --pedal"));
}

#[test]
fn cli_preset_list_json_is_valid() {
    let output = savant()
        .args(["--json", "preset", "--list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("preset --list --json should produce valid JSON");

    // Verify structure
    let presets = json.get("presets").expect("JSON should have presets field");
    assert!(presets.is_array(), "presets should be an array");

    let presets_arr = presets.as_array().unwrap();
    assert!(presets_arr.len() >= 4, "should have at least 4 presets");

    // Verify first preset has required fields
    let first = &presets_arr[0];
    assert!(first.get("name").is_some(), "preset should have name");
    assert!(
        first.get("description").is_some(),
        "preset should have description"
    );
    assert!(first.get("left").is_some(), "preset should have left");
    assert!(first.get("middle").is_some(), "preset should have middle");
    assert!(first.get("right").is_some(), "preset should have right");
}

#[test]
fn cli_preset_show_displays_details() {
    savant()
        .args(["preset", "copy-paste", "--show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PRESET: COPY-PASTE"))
        .stdout(predicate::str::contains(
            "savant program --pedal a --action a --dry-run",
        ));
}

#[test]
fn cli_preset_show_json_is_valid() {
    let output = savant()
        .args(["--json", "preset", "copy-paste", "--show"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("preset --show --json should produce valid JSON");

    assert_eq!(json.get("name").unwrap(), "copy-paste");
    assert_eq!(json.get("left").unwrap(), "cmd+c");
    assert_eq!(json.get("middle").unwrap(), "cmd+a");
    assert_eq!(json.get("right").unwrap(), "cmd+v");
}

#[test]
fn cli_preset_rejects_unknown_name() {
    savant()
        .args(["preset", "invalid-preset-name"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("Unknown preset"))
        .stdout(predicate::str::contains("Available presets"));
}

#[test]
fn cli_preset_missing_name_shows_usage() {
    savant()
        .args(["preset"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Missing preset name"))
        .stdout(predicate::str::contains("savant preset --list"))
        .stdout(predicate::str::contains("does not write the pedal"));
}

#[test]
fn cli_preset_help() {
    savant()
        .args(["preset", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--list"))
        .stdout(predicate::str::contains("--show"))
        .stdout(predicate::str::contains("does not write"))
        .stdout(predicate::str::contains("savant program"))
        .stdout(predicate::str::contains("Apply a preset").not())
        .stdout(predicate::str::contains("without applying").not());
}

#[test]
fn cli_preset_apply_is_unsupported() {
    savant()
        .args(["preset", "browser", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not write the pedal"))
        .stderr(predicate::str::contains("savant program --pedal"));
}

#[test]
fn cli_preset_apply_without_flags_is_unsupported() {
    savant()
        .args(["preset", "copy-paste"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not write the pedal"))
        .stderr(predicate::str::contains("savant program --pedal"));
}

#[test]
fn cli_config_load_is_unsupported() {
    savant()
        .args(["config", "load", "nonexistent-profile-xyz"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not found"));
}

// ============================================================================
// Config Profile Command Tests
// ============================================================================

#[test]
fn cli_config_help() {
    savant()
        .args(["config", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("save"))
        .stdout(predicate::str::contains("load"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("show"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn cli_config_list_shows_profiles_dir() {
    savant()
        .args(["config", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("SAVED PROFILES"))
        .stdout(predicate::str::contains("Profiles directory"));
}

#[test]
fn cli_config_list_json_is_valid() {
    let output = savant()
        .args(["--json", "config", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("config list --json should produce valid JSON");

    assert!(
        json.get("profiles").is_some(),
        "JSON should have profiles field"
    );
    assert!(
        json.get("profiles_dir").is_some(),
        "JSON should have profiles_dir field"
    );
}

#[test]
fn cli_config_save_rejects_invalid_name() {
    savant()
        .args(["config", "save", "invalid/name"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "letters, numbers, hyphens, and underscores",
        ));
}

#[test]
fn cli_config_save_rejects_empty_name() {
    savant()
        .args(["config", "save", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be empty"));
}

#[test]
fn cli_config_save_rejects_leading_underscore() {
    // Test leading underscore rejection (leading hyphen gets interpreted as a flag by clap)
    savant()
        .args(["config", "save", "_myprofile"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot start with"));
}

#[test]
fn cli_config_show_rejects_unknown_profile() {
    savant()
        .args(["config", "show", "nonexistent-profile-xyz"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not found"));
}

#[test]
fn cli_config_load_rejects_unknown_profile() {
    savant()
        .args(["config", "load", "nonexistent-profile-xyz"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not found"));
}

#[test]
fn cli_config_delete_warns_without_force() {
    // Delete without --force should warn (for non-existent profiles it fails differently)
    // We test the warning behavior by trying to delete a non-existent profile
    savant()
        .args(["config", "delete", "nonexistent-profile-xyz"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("not found"));
}

#[test]
fn cli_config_save_help() {
    savant()
        .args(["config", "save", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("Overwrite"));
}

#[test]
fn cli_config_load_help() {
    savant()
        .args(["config", "load", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("does not write the pedal"))
        .stdout(predicate::str::contains("programs device").not());
}

#[test]
fn cli_config_delete_help() {
    savant()
        .args(["config", "delete", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("confirmation"));
}

// ============================================================================
// Doctor Command Tests
// ============================================================================

#[test]
fn cli_doctor_runs_successfully() {
    savant()
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("SYSTEM DIAGNOSTICS"))
        .stdout(predicate::str::contains("Binary"))
        .stdout(predicate::str::contains("Platform"))
        .stdout(predicate::str::contains("SUMMARY"));
}

#[test]
fn cli_doctor_shows_version() {
    savant()
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("savant-elite version"));
}

#[test]
fn cli_doctor_checks_platform() {
    let output = savant()
        .args(["doctor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&output);
    assert!(
        stdout.contains("macOS detected")
            || stdout.contains("Windows detected")
            || stdout.contains("linux detected"),
        "Expected platform detection message in stdout: {}",
        stdout
    );
}

#[cfg(target_os = "windows")]
#[test]
fn cli_doctor_windows_platform_is_supported() {
    let output = savant()
        .args(["--json", "doctor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("doctor --json should produce valid JSON");
    let checks = json
        .get("checks")
        .and_then(|value| value.as_array())
        .expect("doctor JSON should include checks");
    let platform = checks
        .iter()
        .find(|check| check.get("name").and_then(|name| name.as_str()) == Some("platform"))
        .expect("doctor JSON should include a platform check");

    assert_eq!(
        platform.get("status").and_then(|status| status.as_str()),
        Some("pass")
    );
    let message = platform
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    assert!(message.contains("Windows detected"));
    assert!(message.contains("supported"));
    assert!(!message.contains("unsupported"));

    let suggestions = platform
        .get("suggestions")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let joined = suggestions
        .iter()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(joined.contains("05F3:0232"));
    assert!(joined.contains("05F3:030C"));
    assert!(joined.contains("Never bind Play PID"));
}

#[test]
fn cli_doctor_json_is_valid() {
    let output = savant()
        .args(["--json", "doctor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value =
        serde_json::from_slice(&output).expect("doctor --json should produce valid JSON");

    // Verify structure
    assert!(json.get("version").is_some(), "JSON should have version");
    assert!(json.get("platform").is_some(), "JSON should have platform");
    assert!(json.get("arch").is_some(), "JSON should have arch");
    assert!(json.get("checks").is_some(), "JSON should have checks");
    assert!(json.get("summary").is_some(), "JSON should have summary");

    // Verify summary structure
    let summary = json.get("summary").unwrap();
    assert!(summary.get("total").is_some(), "summary should have total");
    assert!(
        summary.get("passed").is_some(),
        "summary should have passed"
    );
    assert!(
        summary.get("warnings").is_some(),
        "summary should have warnings"
    );
    assert!(
        summary.get("failed").is_some(),
        "summary should have failed"
    );
    assert!(
        summary.get("healthy").is_some(),
        "summary should have healthy"
    );
}

#[test]
fn cli_doctor_json_has_checks() {
    let output = savant()
        .args(["--json", "doctor"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let checks = json.get("checks").unwrap().as_array().unwrap();

    // Should have multiple checks
    assert!(checks.len() >= 5, "should have at least 5 checks");

    // Each check should have required fields
    for check in checks {
        assert!(check.get("name").is_some(), "check should have name");
        assert!(check.get("status").is_some(), "check should have status");
        assert!(check.get("message").is_some(), "check should have message");
    }
}

#[test]
fn cli_doctor_help() {
    savant()
        .args(["doctor", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("diagnostics"));
}

// ============================================================================
// Timeout Tests
// ============================================================================

#[test]
fn cli_timeout_help_shows_default() {
    savant()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--timeout <MS>"))
        .stdout(predicate::str::contains("500"));
}

#[test]
fn cli_timeout_accepts_valid_value() {
    savant()
        .args(["--timeout", "1000", "status"])
        .assert()
        .success();
}

#[test]
fn cli_timeout_accepts_minimum_value() {
    savant()
        .args(["--timeout", "100", "status"])
        .assert()
        .success();
}

#[test]
fn cli_timeout_accepts_maximum_value() {
    savant()
        .args(["--timeout", "600000", "status"])
        .assert()
        .success();
}

#[test]
fn cli_timeout_rejects_too_low() {
    savant()
        .args(["--timeout", "50", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("50 is not in 100..=600000"));
}

#[test]
fn cli_timeout_rejects_too_high() {
    savant()
        .args(["--timeout", "700000", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("700000 is not in 100..=600000"));
}

#[test]
fn cli_timeout_rejects_non_numeric() {
    savant()
        .args(["--timeout", "abc", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn cli_timeout_verbose_shows_value() {
    savant()
        .args(["--verbose", "--timeout", "5000", "status"])
        .assert()
        .success()
        .stderr(predicate::str::contains("USB timeout: 5000ms"));
}

// ============================================================================
// Config Check Tests
// ============================================================================

#[test]
fn cli_config_check_help() {
    savant()
        .args(["config", "check", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Validate"))
        .stdout(predicate::str::contains("FILE"));
}

#[test]
fn cli_config_check_valid_config() {
    // This test assumes there's a valid config file on the system
    // If not, it will report file not found which is also valid behavior
    let result = savant().args(["config", "check"]).assert();

    // Either success (valid config) or failure (no config) is OK
    // We just check that it doesn't panic
    let output = result.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should have some meaningful output
    assert!(
        stdout.contains("Configuration")
            || stderr.contains("Configuration")
            || stderr.contains("not found"),
        "Expected some output about configuration"
    );
}

#[test]
fn cli_config_check_nonexistent_file() {
    savant()
        .args(["config", "check", "/nonexistent/file.conf"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("File not found"));
}

#[test]
fn cli_config_check_json_valid() {
    // Test JSON output structure on existing config
    let result = savant().args(["--json", "config", "check"]).assert();

    let output = result.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // If there's a config, should be valid JSON
    if !stdout.is_empty() {
        let json: Result<serde_json::Value, _> = serde_json::from_str(&stdout);
        assert!(json.is_ok(), "JSON output should be valid: {}", stdout);

        let json = json.unwrap();
        assert!(
            json.get("valid").is_some(),
            "JSON should have 'valid' field"
        );
        assert!(json.get("file").is_some(), "JSON should have 'file' field");
    }
}

#[test]
fn cli_config_check_json_nonexistent() {
    let output = savant()
        .args(["--json", "config", "check", "/nonexistent/file.conf"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();

    assert_eq!(json.get("valid").unwrap(), false);
    assert!(!json.get("errors").unwrap().as_array().unwrap().is_empty());
}

#[test]
fn cli_config_check_invalid_file() {
    // Create a temp file with invalid content
    use std::io::Write;
    let mut temp = tempfile::NamedTempFile::new().unwrap();
    writeln!(temp, "left=cmd+invalid_key").unwrap();
    writeln!(temp, "middle=cmd+a").unwrap();
    // missing right

    savant()
        .args(["config", "check", temp.path().to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("Configuration invalid"))
        .stdout(predicate::str::contains("error"));
}

#[test]
fn cli_config_check_verbose() {
    savant()
        .args(["--verbose", "config", "check"])
        .assert()
        .stderr(predicate::str::contains("Checking config file"));
}

// ============================================================================
// Config History and Restore Tests
// ============================================================================

#[test]
fn cli_config_history_shows_help() {
    savant()
        .args(["config", "history", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("automatic backups"));
}

#[test]
fn cli_config_history_runs() {
    savant().args(["config", "history"]).assert().success();
}

#[test]
fn cli_config_history_json_output() {
    let output = savant()
        .args(["--json", "config", "history"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(json.get("history").is_some());
    assert!(json.get("count").is_some());
    assert!(json.get("history_dir").is_some());
}

#[test]
fn cli_config_history_verbose() {
    savant()
        .args(["--verbose", "config", "history"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Listing config history"));
}

#[test]
fn cli_config_restore_shows_help() {
    savant()
        .args(["config", "restore", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Backup number"))
        .stdout(predicate::str::contains("--apply"))
        .stdout(predicate::str::contains("does not write the pedal"))
        .stdout(predicate::str::contains("savant program"))
        .stdout(predicate::str::contains("Program device immediately").not());
}

#[test]
fn cli_config_restore_requires_number() {
    savant()
        .args(["config", "restore"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("NUMBER"));
}

#[test]
fn cli_config_restore_rejects_zero() {
    savant()
        .args(["config", "restore", "0"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Invalid backup number").or(predicate::str::contains(
                "No configuration history available",
            )),
        );
}

#[test]
fn cli_config_restore_rejects_out_of_range() {
    savant()
        .args(["config", "restore", "9999"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Invalid backup number").or(predicate::str::contains(
                "No configuration history available",
            )),
        );
}
