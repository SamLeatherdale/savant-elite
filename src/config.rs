//! On-disk pedal configuration, named profiles, and built-in presets.

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::cli::{
    ConfigCommand, JsonConfigCheckError, JsonConfigCheckOutput, JsonConfigCheckParsedKey,
    JsonPreset, JsonPresetListOutput, JsonProfile, JsonProfileDeleteOutput, JsonProfileListOutput,
    JsonProfileSaveOutput, SavantElite,
};
use crate::protocol::KeyAction;

/// Pedal configuration stored on disk (device EEPROM is write-only)
#[derive(Clone)]
pub struct PedalConfig {
    pub left: String,
    pub middle: String,
    pub right: String,
}

impl PedalConfig {
    pub fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("savant-elite");
        config_dir.join("pedals.conf")
    }

    pub fn parse(content: &str) -> Option<Self> {
        let mut left = String::new();
        let mut middle = String::new();
        let mut right = String::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let key = key.trim();
            let value = value.trim();

            match key {
                "left" => left = value.to_string(),
                "middle" => middle = value.to_string(),
                "right" => right = value.to_string(),
                _ => {}
            }
        }

        if !left.is_empty() && !middle.is_empty() && !right.is_empty() {
            Some(Self {
                left,
                middle,
                right,
            })
        } else {
            None
        }
    }

    pub fn load_from(path: &std::path::Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        Self::parse(&content)
    }

    pub fn load() -> Option<Self> {
        Self::load_from(&Self::config_path())
    }

    pub fn serialize(&self) -> Result<String> {
        // Validate no newlines in values (would corrupt config file format)
        for (name, val) in [
            ("left", &self.left),
            ("middle", &self.middle),
            ("right", &self.right),
        ] {
            if val.contains('\n') || val.contains('\r') {
                return Err(anyhow!(
                    "Key action for {} contains invalid newline character",
                    name
                ));
            }
        }

        Ok(format!(
            "left={}\nmiddle={}\nright={}\n",
            self.left, self.middle, self.right
        ))
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        let content = self.serialize()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, content)?;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        // Backup current config before overwriting (if it exists)
        Self::backup_current_config();
        self.save_to(&Self::config_path())
    }

    /// Get the history directory path for config backups
    pub fn history_dir() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("savant-elite");
        config_dir.join("history")
    }

    /// Backup current config to history directory with timestamp
    fn backup_current_config() {
        let config_path = Self::config_path();
        if !config_path.exists() {
            return; // Nothing to backup
        }

        let history_dir = Self::history_dir();
        if fs::create_dir_all(&history_dir).is_err() {
            return; // Can't create history dir, skip backup silently
        }

        // Generate timestamp-based filename
        let now = chrono::Local::now();
        let backup_name = format!("{}.conf", now.format("%Y-%m-%d_%H%M%S"));
        let backup_path = history_dir.join(&backup_name);

        // Copy current config to backup
        if fs::copy(&config_path, &backup_path).is_ok() {
            // Prune old backups after successful backup
            Self::prune_old_backups();
        }
    }

    /// Keep only the most recent N backups (default 10)
    fn prune_old_backups() {
        let max_backups: usize = std::env::var("SAVANT_HISTORY_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let history_dir = Self::history_dir();
        if !history_dir.exists() {
            return;
        }

        // Collect backup files
        let mut backups: Vec<PathBuf> = fs::read_dir(&history_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "conf"))
            .collect();

        // Sort by filename (timestamp format ensures chronological order)
        backups.sort();

        // Remove oldest backups if we have too many
        if backups.len() > max_backups {
            let to_remove = backups.len() - max_backups;
            for backup in backups.iter().take(to_remove) {
                let _ = fs::remove_file(backup);
            }
        }
    }

    /// List all backup files with timestamps and config summaries
    pub fn list_backups() -> Vec<(PathBuf, chrono::NaiveDateTime, Option<Self>)> {
        let history_dir = Self::history_dir();
        if !history_dir.exists() {
            return Vec::new();
        }

        let mut backups: Vec<(PathBuf, chrono::NaiveDateTime, Option<Self>)> =
            fs::read_dir(&history_dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "conf"))
                .filter_map(|path| {
                    // Parse timestamp from filename (YYYY-MM-DD_HHMMSS.conf)
                    let stem = path.file_stem()?.to_str()?;
                    let datetime =
                        chrono::NaiveDateTime::parse_from_str(stem, "%Y-%m-%d_%H%M%S").ok()?;
                    let config = Self::load_from(&path);
                    Some((path, datetime, config))
                })
                .collect();

        // Sort by timestamp, newest first
        backups.sort_by_key(|b| std::cmp::Reverse(b.1));
        backups
    }

    /// Restore a backup by index (1 = most recent)
    pub fn restore_backup(index: usize) -> Result<Self> {
        let backups = Self::list_backups();

        if backups.is_empty() {
            return Err(anyhow!(
                "No configuration history available. History is created when you program the device or load a profile."
            ));
        }

        if index == 0 || index > backups.len() {
            return Err(anyhow!(
                "Invalid backup number {}. Valid range: 1-{}",
                index,
                backups.len()
            ));
        }

        let (path, _, config) = &backups[index - 1];

        config
            .clone()
            .ok_or_else(|| anyhow!("Failed to parse backup file: {}", path.display()))
    }
}

/// Built-in preset configurations for common use cases
#[derive(Clone)]
pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub left: &'static str,
    pub middle: &'static str,
    pub right: &'static str,
}

impl Preset {
    pub const fn new(
        name: &'static str,
        description: &'static str,
        left: &'static str,
        middle: &'static str,
        right: &'static str,
    ) -> Self {
        Self {
            name,
            description,
            left,
            middle,
            right,
        }
    }
}

/// Built-in presets - curated for common workflows
pub const PRESETS: &[Preset] = &[
    Preset::new(
        "copy-paste",
        "Copy/Select/Paste workflow - the most universally useful configuration",
        "cmd+c",
        "cmd+a",
        "cmd+v",
    ),
    Preset::new(
        "undo-redo",
        "Undo/Select/Redo workflow for editing",
        "cmd+z",
        "cmd+a",
        "shift+cmd+z",
    ),
    Preset::new(
        "browser",
        "Browser navigation - back/new tab/forward",
        "cmd+[",
        "cmd+t",
        "cmd+]",
    ),
    Preset::new(
        "zoom",
        "Zoom video calls - mute/video/leave (macOS shortcuts)",
        "cmd+shift+a",
        "cmd+shift+v",
        "cmd+w",
    ),
];

pub fn find_preset(name: &str) -> Option<&'static Preset> {
    let name_lower = name.to_lowercase();
    PRESETS.iter().find(|p| p.name == name_lower)
}

/// Get the profiles directory path
pub fn profiles_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("savant-elite")
        .join("profiles")
}

/// Get the path for a specific profile
pub fn profile_path(name: &str) -> PathBuf {
    profiles_dir().join(format!("{}.conf", name))
}

/// Validate profile name (alphanumeric, hyphen, underscore only)
pub fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("Profile name cannot be empty"));
    }
    if name.len() > 64 {
        return Err(anyhow!("Profile name too long (max 64 characters)"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!(
            "Profile name can only contain letters, numbers, hyphens, and underscores"
        ));
    }
    // Prevent names that could cause issues
    if name.starts_with('-') || name.starts_with('_') {
        return Err(anyhow!(
            "Profile name cannot start with a hyphen or underscore"
        ));
    }
    Ok(())
}

impl SavantElite {
    pub(crate) fn preset(
        &self,
        name: Option<&str>,
        list: bool,
        show: bool,
        dry_run: bool,
    ) -> Result<()> {
        // Handle --list flag
        if list {
            return self.list_presets();
        }

        // All other operations require a preset name
        let Some(preset_name) = name else {
            if self.json_output {
                let output = JsonPresetListOutput {
                    presets: PRESETS
                        .iter()
                        .map(|p| JsonPreset {
                            name: p.name.to_string(),
                            description: p.description.to_string(),
                            left: p.left.to_string(),
                            middle: p.middle.to_string(),
                            right: p.right.to_string(),
                        })
                        .collect(),
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                self.console
                    .print("[bold red]Error:[/] Missing preset name");
                self.console.print("");
                self.console
                    .print("Usage: [bold yellow]savant preset --list[/] or [bold yellow]savant preset <NAME> --show[/]");
                self.console.print("");
                self.console.print(
                    "This command does not write the pedal. Use [bold yellow]savant program --pedal a --action a --dry-run[/].",
                );
            }
            return Ok(());
        };

        // Look up the preset
        let Some(preset) = find_preset(preset_name) else {
            if self.json_output {
                let err = serde_json::json!({
                    "error": "unknown_preset",
                    "message": format!("Unknown preset: '{}'", preset_name),
                    "available": PRESETS.iter().map(|p| p.name).collect::<Vec<_>>()
                });
                println!("{}", serde_json::to_string_pretty(&err)?);
            } else {
                self.console.print(&format!(
                    "[bold red]Error:[/] Unknown preset: '{}'",
                    preset_name
                ));
                self.console.print("");
                self.console.print("[bold cyan]Available presets:[/]");
                for p in PRESETS {
                    self.console
                        .print(&format!("  [yellow]{}[/]  {}", p.name, p.description));
                }
            }
            return Err(anyhow!("Unknown preset: '{}'", preset_name));
        };

        // Handle --show flag
        if show {
            return self.show_preset(preset);
        }

        let _ = dry_run;
        self.verbose(&format!("Refusing preset apply: {}", preset.name));
        self.refuse_unverified_apply(&format!("savant preset {}", preset.name))
    }

    fn list_presets(&self) -> Result<()> {
        if self.json_output {
            let output = JsonPresetListOutput {
                presets: PRESETS
                    .iter()
                    .map(|p| JsonPreset {
                        name: p.name.to_string(),
                        description: p.description.to_string(),
                        left: p.left.to_string(),
                        middle: p.middle.to_string(),
                        right: p.right.to_string(),
                    })
                    .collect(),
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        self.print_banner();

        self.console.print(
            "[bold #3498db]┌─────────────────────────────────────────────────────────────────┐[/]",
        );
        self.console.print(
            "[bold #3498db]│[/]  [bold white]AVAILABLE PRESETS[/]                                           [bold #3498db]│[/]",
        );
        self.console.print(
            "[bold #3498db]└─────────────────────────────────────────────────────────────────┘[/]",
        );
        self.console.print("");

        for preset in PRESETS {
            self.console
                .print(&format!("  [bold yellow]{}[/]", preset.name));
            self.console
                .print(&format!("    [dim]{}[/]", preset.description));
            self.console.print(&format!(
                "    Left: [cyan]{}[/]  Middle: [cyan]{}[/]  Right: [cyan]{}[/]",
                preset.left, preset.middle, preset.right
            ));
            self.console.print("");
        }

        self.console.print("[bold #2ecc71]Usage:[/]");
        self.console
            .print("  [yellow]savant preset --list[/]            List built-in mapping names");
        self.console
            .print("  [yellow]savant preset zoom --show[/]       Show a preset without writing");
        self.console
            .print("  This command does not write the pedal. Write one mapping with:");
        self.console
            .print("  [yellow]savant program --pedal a --action a --dry-run[/]");

        Ok(())
    }

    fn show_preset(&self, preset: &Preset) -> Result<()> {
        if self.json_output {
            let output = JsonPreset {
                name: preset.name.to_string(),
                description: preset.description.to_string(),
                left: preset.left.to_string(),
                middle: preset.middle.to_string(),
                right: preset.right.to_string(),
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        self.print_banner();

        self.console.print(
            "[bold #9b59b6]┌─────────────────────────────────────────────────────────────────┐[/]",
        );
        self.console.print(&format!(
            "[bold #9b59b6]│[/]  [bold white]PRESET: {}[/]{}[bold #9b59b6]│[/]",
            preset.name.to_uppercase(),
            " ".repeat(53_usize.saturating_sub(preset.name.len()))
        ));
        self.console.print(
            "[bold #9b59b6]└─────────────────────────────────────────────────────────────────┘[/]",
        );
        self.console.print("");

        self.console
            .print(&format!("  [dim]{}[/]", preset.description));
        self.console.print("");

        // Show the pedal visualization
        self.print_pedal_visualization(preset.left, preset.middle, preset.right);

        self.console.print("");
        self.console
            .print("This command does not write the pedal. Write one mapping with:");
        self.console
            .print("  [bold yellow]savant program --pedal a --action a --dry-run[/]");

        Ok(())
    }

    pub(crate) fn config(&self, command: ConfigCommand) -> Result<()> {
        match command {
            ConfigCommand::Save { name, force } => self.config_save(&name, force),
            ConfigCommand::Load { name, dry_run } => self.config_load(&name, dry_run),
            ConfigCommand::List => self.config_list(),
            ConfigCommand::Show { name } => self.config_show(&name),
            ConfigCommand::Delete { name, force } => self.config_delete(&name, force),
            ConfigCommand::Check { file } => self.config_check(file.as_deref()),
            ConfigCommand::History => self.config_history(),
            ConfigCommand::Restore { number, apply } => self.config_restore(number, apply),
        }
    }

    fn config_save(&self, name: &str, force: bool) -> Result<()> {
        // Validate profile name
        validate_profile_name(name)?;

        self.verbose(&format!("Saving profile: {}", name));

        // Check if current config exists
        let current_config = PedalConfig::load();
        let Some(config) = current_config else {
            if self.json_output {
                let err = serde_json::json!({
                    "error": "no_current_config",
                    "message": "No current configuration to save. Run 'savant program' first."
                });
                println!("{}", serde_json::to_string_pretty(&err)?);
            } else {
                self.console
                    .print("[bold red]Error:[/] No current configuration to save.");
                self.console.print("");
                self.console
                    .print("Run [bold yellow]savant program[/] first to create a configuration.");
            }
            return Err(anyhow!("No current configuration to save"));
        };

        let path = profile_path(name);

        // Check if profile already exists
        if path.exists() && !force {
            if self.json_output {
                let err = serde_json::json!({
                    "error": "profile_exists",
                    "message": format!("Profile '{}' already exists. Use --force to overwrite.", name),
                    "path": path.display().to_string()
                });
                println!("{}", serde_json::to_string_pretty(&err)?);
            } else {
                self.console.print(&format!(
                    "[bold red]Error:[/] Profile '{}' already exists.",
                    name
                ));
                self.console.print("");
                self.console.print(&format!(
                    "Use [bold yellow]savant config save {} --force[/] to overwrite.",
                    name
                ));
            }
            return Err(anyhow!("Profile '{}' already exists", name));
        }

        // Create profiles directory if needed
        let dir = profiles_dir();
        if !dir.exists() {
            self.verbose(&format!("Creating profiles directory: {}", dir.display()));
            fs::create_dir_all(&dir).context("Failed to create profiles directory")?;
        }

        // Save the profile
        config.save_to(&path).context("Failed to save profile")?;

        if self.json_output {
            let output = JsonProfileSaveOutput {
                success: true,
                name: name.to_string(),
                path: path.display().to_string(),
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            self.console.print(&format!(
                "[bold #2ecc71]✓[/] Saved profile '[bold yellow]{}[/]'",
                name
            ));
            self.console
                .print(&format!("  Path: [dim]{}[/]", path.display()));
            self.console.print("");
            self.console.print(&format!(
                "Show later: [bold yellow]savant config show {}[/]",
                name
            ));
            self.console.print(
                "  To write the pedal, use [bold yellow]savant program --pedal a --action a --dry-run[/]",
            );
        }

        Ok(())
    }

    fn config_load(&self, name: &str, dry_run: bool) -> Result<()> {
        // Validate profile name
        validate_profile_name(name)?;

        self.verbose(&format!("Loading profile: {}", name));

        let path = profile_path(name);

        // Check if profile exists
        if !path.exists() {
            if self.json_output {
                let err = serde_json::json!({
                    "error": "profile_not_found",
                    "message": format!("Profile '{}' not found.", name),
                    "path": path.display().to_string()
                });
                println!("{}", serde_json::to_string_pretty(&err)?);
            } else {
                self.console.print(&format!(
                    "[bold red]Error:[/] Profile '{}' not found.",
                    name
                ));
                self.console.print("");
                self.console
                    .print("Run [bold yellow]savant config list[/] to see available profiles.");
            }
            return Err(anyhow!("Profile '{}' not found", name));
        }

        // Load the profile
        let config = PedalConfig::load_from(&path)
            .ok_or_else(|| anyhow!("Failed to parse profile '{}'", name))?;

        self.verbose(&format!(
            "Profile contents: left={}, middle={}, right={}",
            config.left, config.middle, config.right
        ));

        let _ = dry_run;
        self.verbose(&format!("Refusing config load apply: {}", name));
        self.refuse_unverified_apply(&format!("savant config load {}", name))
    }

    fn config_list(&self) -> Result<()> {
        self.verbose("Listing profiles");

        let dir = profiles_dir();

        // Collect profiles
        let mut profiles: Vec<(String, PedalConfig)> = Vec::new();

        if dir.exists() {
            for entry in fs::read_dir(&dir).context("Failed to read profiles directory")? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "conf") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        if let Some(config) = PedalConfig::load_from(&path) {
                            profiles.push((name.to_string(), config));
                        }
                    }
                }
            }
        }

        // Sort by name
        profiles.sort_by(|a, b| a.0.cmp(&b.0));

        if self.json_output {
            let output = JsonProfileListOutput {
                profiles: profiles
                    .iter()
                    .map(|(name, config)| JsonProfile {
                        name: name.clone(),
                        left: config.left.clone(),
                        middle: config.middle.clone(),
                        right: config.right.clone(),
                    })
                    .collect(),
                profiles_dir: dir.display().to_string(),
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        self.print_banner();

        self.console.print(
            "[bold #3498db]┌─────────────────────────────────────────────────────────────────┐[/]",
        );
        self.console.print(
            "[bold #3498db]│[/]  [bold white]SAVED PROFILES[/]                                              [bold #3498db]│[/]",
        );
        self.console.print(
            "[bold #3498db]└─────────────────────────────────────────────────────────────────┘[/]",
        );
        self.console.print("");

        if profiles.is_empty() {
            self.console.print("  [dim]No saved profiles yet.[/]");
            self.console.print("");
            self.console
                .print("  Create one with: [bold yellow]savant config save <name>[/]");
        } else {
            for (name, config) in &profiles {
                self.console.print(&format!("  [bold yellow]{}[/]", name));
                self.console.print(&format!(
                    "    Left: [cyan]{}[/]  Middle: [cyan]{}[/]  Right: [cyan]{}[/]",
                    config.left, config.middle, config.right
                ));
                self.console.print("");
            }
        }

        self.console
            .print(&format!("[dim]Profiles directory: {}[/]", dir.display()));

        Ok(())
    }

    fn config_show(&self, name: &str) -> Result<()> {
        // Validate profile name
        validate_profile_name(name)?;

        self.verbose(&format!("Showing profile: {}", name));

        let path = profile_path(name);

        // Check if profile exists
        if !path.exists() {
            if self.json_output {
                let err = serde_json::json!({
                    "error": "profile_not_found",
                    "message": format!("Profile '{}' not found.", name),
                    "path": path.display().to_string()
                });
                println!("{}", serde_json::to_string_pretty(&err)?);
            } else {
                self.console.print(&format!(
                    "[bold red]Error:[/] Profile '{}' not found.",
                    name
                ));
                self.console.print("");
                self.console
                    .print("Run [bold yellow]savant config list[/] to see available profiles.");
            }
            return Err(anyhow!("Profile '{}' not found", name));
        }

        // Load the profile
        let config = PedalConfig::load_from(&path)
            .ok_or_else(|| anyhow!("Failed to parse profile '{}'", name))?;

        if self.json_output {
            let output = JsonProfile {
                name: name.to_string(),
                left: config.left.clone(),
                middle: config.middle.clone(),
                right: config.right.clone(),
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        self.print_banner();

        self.console.print(
            "[bold #9b59b6]┌─────────────────────────────────────────────────────────────────┐[/]",
        );
        self.console.print(&format!(
            "[bold #9b59b6]│[/]  [bold white]PROFILE: {}[/]{}[bold #9b59b6]│[/]",
            name.to_uppercase(),
            " ".repeat(55_usize.saturating_sub(name.len()))
        ));
        self.console.print(
            "[bold #9b59b6]└─────────────────────────────────────────────────────────────────┘[/]",
        );
        self.console.print("");

        // Show the pedal visualization
        self.print_pedal_visualization(&config.left, &config.middle, &config.right);

        self.console.print("");
        self.console
            .print("This command does not write the pedal. Write one mapping with:");
        self.console
            .print("  [bold yellow]savant program --pedal a --action a --dry-run[/]");

        Ok(())
    }

    fn config_delete(&self, name: &str, force: bool) -> Result<()> {
        // Validate profile name
        validate_profile_name(name)?;

        self.verbose(&format!("Deleting profile: {}", name));

        let path = profile_path(name);

        // Check if profile exists
        if !path.exists() {
            if self.json_output {
                let err = serde_json::json!({
                    "error": "profile_not_found",
                    "message": format!("Profile '{}' not found.", name),
                    "path": path.display().to_string()
                });
                println!("{}", serde_json::to_string_pretty(&err)?);
            } else {
                self.console.print(&format!(
                    "[bold red]Error:[/] Profile '{}' not found.",
                    name
                ));
            }
            return Err(anyhow!("Profile '{}' not found", name));
        }

        // In JSON mode or with --force, just delete
        // Without --force in interactive mode, we'd ideally prompt - but CLI tools
        // typically use --force for this. We'll just require --force for safety.
        if !force && !self.json_output {
            self.console.print(&format!(
                "[bold #f39c12]Warning:[/] About to delete profile '[bold yellow]{}[/]'",
                name
            ));
            self.console.print("");
            self.console.print(&format!(
                "Use [bold yellow]savant config delete {} --force[/] to confirm.",
                name
            ));
            return Ok(());
        }

        // Delete the profile
        fs::remove_file(&path).context("Failed to delete profile")?;

        if self.json_output {
            let output = JsonProfileDeleteOutput {
                success: true,
                name: name.to_string(),
                path: path.display().to_string(),
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            self.console.print(&format!(
                "[bold #2ecc71]✓[/] Deleted profile '[bold yellow]{}[/]'",
                name
            ));
        }

        Ok(())
    }

    fn config_check(&self, file: Option<&str>) -> Result<()> {
        self.verbose(&format!(
            "Checking config file: {}",
            file.unwrap_or("(default)")
        ));

        // Determine which file to check
        let config_path = match file {
            Some(path) => std::path::PathBuf::from(path),
            None => PedalConfig::config_path(),
        };

        let path_display = config_path.display().to_string();

        // Check if file exists
        if !config_path.exists() {
            let error = JsonConfigCheckError {
                line: None,
                field: None,
                value: None,
                error: "File not found".to_string(),
            };

            if self.json_output {
                let output = JsonConfigCheckOutput {
                    valid: false,
                    file: path_display.clone(),
                    left: None,
                    middle: None,
                    right: None,
                    errors: vec![error],
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                self.console.print("[bold red]✗[/] Configuration invalid");
                self.console.print("");
                self.console.print(&format!(
                    "  [bold red]Error:[/] File not found: {}",
                    path_display
                ));
                self.console.print("");
                self.console
                    .print("  [dim]Run 'savant program' to create a configuration.[/]");
            }
            return Err(anyhow!("Config file not found: {}", path_display));
        }

        // Read file content
        let content = match fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(e) => {
                let error = JsonConfigCheckError {
                    line: None,
                    field: None,
                    value: None,
                    error: format!("Cannot read file: {}", e),
                };

                if self.json_output {
                    let output = JsonConfigCheckOutput {
                        valid: false,
                        file: path_display.clone(),
                        left: None,
                        middle: None,
                        right: None,
                        errors: vec![error],
                    };
                    println!("{}", serde_json::to_string_pretty(&output)?);
                } else {
                    self.console.print("[bold red]✗[/] Configuration invalid");
                    self.console.print("");
                    self.console
                        .print(&format!("  [bold red]Error:[/] Cannot read file: {}", e));
                }
                return Err(anyhow!("Cannot read config file: {}", e));
            }
        };

        // Parse and validate config
        let mut errors: Vec<JsonConfigCheckError> = Vec::new();
        let mut left_value: Option<String> = None;
        let mut middle_value: Option<String> = None;
        let mut right_value: Option<String> = None;
        let mut left_parsed: Option<KeyAction> = None;
        let mut middle_parsed: Option<KeyAction> = None;
        let mut right_parsed: Option<KeyAction> = None;

        // Parse each line
        for (line_num, line) in content.lines().enumerate() {
            let line_display = line_num + 1; // 1-indexed for display
            let line = line.trim();

            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Check for proper key=value format
            let Some((key, value)) = line.split_once('=') else {
                errors.push(JsonConfigCheckError {
                    line: Some(line_display),
                    field: None,
                    value: Some(line.to_string()),
                    error: "Invalid syntax: expected 'key=value' format".to_string(),
                });
                continue;
            };

            let key = key.trim();
            let value = value.trim();

            // Check for valid field name
            match key {
                "left" => {
                    left_value = Some(value.to_string());
                    match KeyAction::from_string(value) {
                        Ok(action) => left_parsed = Some(action),
                        Err(e) => {
                            errors.push(JsonConfigCheckError {
                                line: Some(line_display),
                                field: Some("left".to_string()),
                                value: Some(value.to_string()),
                                error: e.to_string(),
                            });
                        }
                    }
                }
                "middle" => {
                    middle_value = Some(value.to_string());
                    match KeyAction::from_string(value) {
                        Ok(action) => middle_parsed = Some(action),
                        Err(e) => {
                            errors.push(JsonConfigCheckError {
                                line: Some(line_display),
                                field: Some("middle".to_string()),
                                value: Some(value.to_string()),
                                error: e.to_string(),
                            });
                        }
                    }
                }
                "right" => {
                    right_value = Some(value.to_string());
                    match KeyAction::from_string(value) {
                        Ok(action) => right_parsed = Some(action),
                        Err(e) => {
                            errors.push(JsonConfigCheckError {
                                line: Some(line_display),
                                field: Some("right".to_string()),
                                value: Some(value.to_string()),
                                error: e.to_string(),
                            });
                        }
                    }
                }
                _ => {
                    // Unknown key - warning, not error (for future compatibility)
                    self.verbose(&format!(
                        "Unknown key '{}' at line {} (ignored)",
                        key, line_display
                    ));
                }
            }
        }

        // Check for missing required fields
        if left_value.is_none() {
            errors.push(JsonConfigCheckError {
                line: None,
                field: Some("left".to_string()),
                value: None,
                error: "Missing required field: left".to_string(),
            });
        }
        if middle_value.is_none() {
            errors.push(JsonConfigCheckError {
                line: None,
                field: Some("middle".to_string()),
                value: None,
                error: "Missing required field: middle".to_string(),
            });
        }
        if right_value.is_none() {
            errors.push(JsonConfigCheckError {
                line: None,
                field: Some("right".to_string()),
                value: None,
                error: "Missing required field: right".to_string(),
            });
        }

        // Build output
        let is_valid = errors.is_empty();
        let error_count = errors.len();

        if self.json_output {
            let output = JsonConfigCheckOutput {
                valid: is_valid,
                file: path_display,
                left: left_parsed.as_ref().map(|a| JsonConfigCheckParsedKey {
                    action: left_value.clone().unwrap_or_default(),
                    modifier_hex: format!("0x{:02X}", a.modifiers),
                    key_hex: format!("0x{:02X}", a.key),
                }),
                middle: middle_parsed.as_ref().map(|a| JsonConfigCheckParsedKey {
                    action: middle_value.clone().unwrap_or_default(),
                    modifier_hex: format!("0x{:02X}", a.modifiers),
                    key_hex: format!("0x{:02X}", a.key),
                }),
                right: right_parsed.as_ref().map(|a| JsonConfigCheckParsedKey {
                    action: right_value.clone().unwrap_or_default(),
                    modifier_hex: format!("0x{:02X}", a.modifiers),
                    key_hex: format!("0x{:02X}", a.key),
                }),
                errors,
            };
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else if is_valid {
            self.console.print("[bold #2ecc71]✓[/] Configuration valid");
            self.console.print("");

            if self.verbose {
                self.console
                    .print(&format!("  [dim]File:[/] {}", path_display));
                self.console.print("");
            }

            // Show parsed configuration
            if let (Some(left), Some(left_a)) = (&left_value, &left_parsed) {
                self.console
                    .print(&format!("  [bold #e74c3c]Left:[/]   {}", left));
                if self.verbose {
                    self.console.print(&format!(
                        "          [dim]Parsed: modifier=0x{:02X}, key=0x{:02X}[/]",
                        left_a.modifiers, left_a.key
                    ));
                }
            }
            if let (Some(middle), Some(middle_a)) = (&middle_value, &middle_parsed) {
                self.console
                    .print(&format!("  [bold #f39c12]Middle:[/] {}", middle));
                if self.verbose {
                    self.console.print(&format!(
                        "          [dim]Parsed: modifier=0x{:02X}, key=0x{:02X}[/]",
                        middle_a.modifiers, middle_a.key
                    ));
                }
            }
            if let (Some(right), Some(right_a)) = (&right_value, &right_parsed) {
                self.console
                    .print(&format!("  [bold #2ecc71]Right:[/]  {}", right));
                if self.verbose {
                    self.console.print(&format!(
                        "          [dim]Parsed: modifier=0x{:02X}, key=0x{:02X}[/]",
                        right_a.modifiers, right_a.key
                    ));
                }
            }
        } else {
            self.console.print("[bold red]✗[/] Configuration invalid");
            self.console.print("");

            for error in &errors {
                let mut msg = String::new();
                if let Some(line) = error.line {
                    msg.push_str(&format!("Line {}: ", line));
                }
                // Only show "field=value" when we have both field and value
                // For missing field errors, the error message already explains what's missing
                if let (Some(ref field), Some(ref value)) = (&error.field, &error.value) {
                    msg.push_str(&format!("{}={}", field, value));
                }
                if !msg.is_empty() {
                    self.console.print(&format!("  [dim]{}[/]", msg));
                }
                self.console
                    .print(&format!("  [bold red]Error:[/] {}", error.error));
                self.console.print("");
            }

            self.console
                .print(&format!("[dim]{} error(s) found[/]", errors.len()));
            self.console.print("");
            self.console
                .print("Run [bold yellow]savant keys[/] for a complete list of valid key names.");
        }

        if is_valid {
            Ok(())
        } else {
            Err(anyhow!("Configuration has {} error(s)", error_count))
        }
    }

    fn config_history(&self) -> Result<()> {
        self.verbose("Listing config history");

        let backups = PedalConfig::list_backups();

        if self.json_output {
            let history: Vec<serde_json::Value> = backups
                .iter()
                .enumerate()
                .map(|(i, (path, datetime, config))| {
                    let mut entry = serde_json::json!({
                        "number": i + 1,
                        "timestamp": datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
                        "path": path.display().to_string(),
                    });
                    if let Some(cfg) = config {
                        entry["left"] = serde_json::json!(cfg.left);
                        entry["middle"] = serde_json::json!(cfg.middle);
                        entry["right"] = serde_json::json!(cfg.right);
                    }
                    entry
                })
                .collect();

            let output = serde_json::json!({
                "history": history,
                "count": backups.len(),
                "history_dir": PedalConfig::history_dir().display().to_string(),
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }

        if backups.is_empty() {
            self.console
                .print("[bold yellow]No configuration history found.[/]");
            self.console.print("");
            self.console
                .print("[dim]History is created automatically when you program the device[/]");
            self.console
                .print("[dim]or load a profile. Each change creates a backup.[/]");
            return Ok(());
        }

        self.console
            .print("[bold #3498db]CONFIG HISTORY[/] [dim](most recent first)[/]");
        self.console.print("");

        for (i, (_path, datetime, config)) in backups.iter().enumerate() {
            let num = format!("{:>3}.", i + 1);
            let timestamp = datetime.format("%Y-%m-%d %H:%M:%S").to_string();

            let summary = if let Some(cfg) = config {
                format!("{}, {}, {}", cfg.left, cfg.middle, cfg.right)
            } else {
                "[dim]<unable to parse>[/]".to_string()
            };

            self.console.print(&format!(
                "  [bold #3498db]{}[/]  [#95a5a6]{}[/]  {}",
                num, timestamp, summary
            ));
        }

        self.console.print("");
        self.console
            .print("[dim]Use 'savant config restore <N>' to restore a previous config.[/]");
        self.console.print(
            "[dim]This command does not write the pedal. Use savant program --pedal a --action a --dry-run.[/]",
        );

        Ok(())
    }

    fn config_restore(&self, number: usize, apply: bool) -> Result<()> {
        self.verbose(&format!("Restoring backup #{}", number));

        let config = PedalConfig::restore_backup(number)?;

        // JSON output only when NOT applying (program() doesn't support JSON mode)
        if self.json_output && !apply {
            let output = serde_json::json!({
                "restored": true,
                "backup_number": number,
                "left": config.left,
                "middle": config.middle,
                "right": config.right,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);

            // Save the restored config
            config.save()?;
            return Ok(());
        }

        self.console.print(&format!(
            "[bold #2ecc71]Restoring configuration from backup #{}...[/]",
            number
        ));
        self.console.print("");
        self.console
            .print(&format!("  [bold]Left:[/]   {}", config.left));
        self.console
            .print(&format!("  [bold]Middle:[/] {}", config.middle));
        self.console
            .print(&format!("  [bold]Right:[/]  {}", config.right));
        self.console.print("");

        if apply {
            self.refuse_unverified_apply("savant config restore --apply")
        } else {
            // Save the restored config
            config.save()?;
            self.console
                .print("[bold #2ecc71]✓[/] Config restored to [bold]pedals.conf[/]");
            self.console.print("");
            self.console
                .print("[dim]This command does not write the pedal. Use savant program --pedal a --action a --dry-run.[/]");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn pedal_config_rejects_newline_in_value() {
        let config = PedalConfig {
            left: "cmd+c\nright=evil".to_string(),
            middle: "cmd+a".to_string(),
            right: "cmd+v".to_string(),
        };
        let err = config.save().unwrap_err();
        assert!(err.to_string().contains("newline"));
    }

    #[test]
    fn pedal_config_rejects_carriage_return_in_value() {
        let config = PedalConfig {
            left: "cmd+c".to_string(),
            middle: "cmd+a\rright=evil".to_string(),
            right: "cmd+v".to_string(),
        };
        let err = config.save().unwrap_err();
        assert!(err.to_string().contains("newline"));
    }

    #[test]
    fn pedal_config_roundtrip() {
        let config = PedalConfig {
            left: "cmd+c".to_string(),
            middle: "cmd+a".to_string(),
            right: "cmd+v".to_string(),
        };

        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("roundtrip.conf");
        config.save_to(&path).unwrap();

        let loaded = PedalConfig::load_from(&path).unwrap();
        assert_eq!(loaded.left, config.left);
        assert_eq!(loaded.middle, config.middle);
        assert_eq!(loaded.right, config.right);
    }

    #[test]
    fn pedal_config_load_returns_none_for_missing_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("missing.conf");
        assert!(PedalConfig::load_from(&path).is_none());
    }

    #[test]
    fn pedal_config_load_returns_none_for_partial_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("partial.conf");
        fs::write(&path, "left=cmd+c\nmiddle=cmd+a\n").unwrap();
        assert!(PedalConfig::load_from(&path).is_none());
    }

    #[test]
    fn pedal_config_load_handles_extra_whitespace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("whitespace.conf");
        fs::write(
            &path,
            "  left =  cmd+c  \n\n middle=  cmd+a\n right\t=\tcmd+v  \nunknown=foo\n",
        )
        .unwrap();
        let loaded = PedalConfig::load_from(&path).unwrap();

        assert_eq!(loaded.left, "cmd+c");
        assert_eq!(loaded.middle, "cmd+a");
        assert_eq!(loaded.right, "cmd+v");
    }
}
