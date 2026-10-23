//! User settings stored as JSON.
//!
//! Missing fields take defaults, so older files keep working after an upgrade. A file that cannot be
//! read is moved aside as `settings.json.bak` and defaults are used, with a notice for the user.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    pub width: f32,
    pub height: f32,
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        WindowState { width: 1280.0, height: 800.0, maximized: false }
    }
}

/// What the app does when it starts without a plan on the command line.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Startup {
    #[default]
    Welcome,
    /// Open the most recently used plan again.
    LastPlan,
}

pub const HEAP_RANGE_MB: (u32, u32) = (64, 65_536);
pub const SUMMARY_INTERVAL_RANGE: (u32, u32) = (1, 60);

pub const MAX_RECENT: usize = 10;
pub const SIDEBAR_RANGE: (f32, f32) = (180.0, 600.0);
pub const INSPECTOR_RANGE: (f32, f32) = (220.0, 600.0);
pub const BOTTOM_RANGE: (f32, f32) = (120.0, 700.0);
pub const WINDOW_MIN: (f32, f32) = (800.0, 500.0);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeChoice,
    pub sidebar_visible: bool,
    pub sidebar_width: f32,
    pub inspector_visible: bool,
    pub inspector_width: f32,
    pub bottom_visible: bool,
    pub bottom_height: f32,
    pub window: WindowState,
    /// Folder of the JMeter install (the one containing `bin`).
    pub jmeter_home: Option<String>,
    /// Path to `java`; when unset the app looks at `JAVA_HOME` and `PATH`.
    pub java_path: Option<String>,
    /// Most recent first.
    pub recent_files: Vec<String>,
    pub startup: Startup,
    /// Java heap size for runs, in megabytes (JMeter's own default is 1 GB).
    pub heap_mb: u32,
    /// Seconds between live summary lines while a run is going.
    pub summary_interval_secs: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: ThemeChoice::System,
            sidebar_visible: true,
            sidebar_width: 280.0,
            inspector_visible: false,
            inspector_width: 300.0,
            bottom_visible: false,
            bottom_height: 240.0,
            window: WindowState::default(),
            jmeter_home: None,
            java_path: None,
            recent_files: Vec::new(),
            startup: Startup::Welcome,
            heap_mb: 1024,
            summary_interval_secs: 1,
        }
    }
}

fn clamp(v: f32, (lo, hi): (f32, f32), fallback: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { fallback }
}

impl Settings {
    /// Bring every value into its allowed range. Called after loading and before saving.
    pub fn sanitize(&mut self) {
        let d = Settings::default();
        self.sidebar_width = clamp(self.sidebar_width, SIDEBAR_RANGE, d.sidebar_width);
        self.inspector_width = clamp(self.inspector_width, INSPECTOR_RANGE, d.inspector_width);
        self.bottom_height = clamp(self.bottom_height, BOTTOM_RANGE, d.bottom_height);
        self.window.width = clamp(self.window.width, (WINDOW_MIN.0, 10_000.0), d.window.width);
        self.window.height = clamp(self.window.height, (WINDOW_MIN.1, 10_000.0), d.window.height);
        self.recent_files.truncate(MAX_RECENT);
        self.heap_mb = self.heap_mb.clamp(HEAP_RANGE_MB.0, HEAP_RANGE_MB.1);
        self.summary_interval_secs = self.summary_interval_secs.clamp(SUMMARY_INTERVAL_RANGE.0, SUMMARY_INTERVAL_RANGE.1);
    }

    /// Put `path` at the front of the recent list, removing an earlier copy.
    pub fn push_recent(&mut self, path: &str) {
        self.recent_files.retain(|p| !same_path(p, path));
        self.recent_files.insert(0, path.to_string());
        self.recent_files.truncate(MAX_RECENT);
    }

    pub fn remove_recent(&mut self, path: &str) {
        self.recent_files.retain(|p| !same_path(p, path));
    }
}

fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").to_lowercase();
    norm(a) == norm(b)
}

pub struct Loaded {
    pub settings: Settings,
    /// Something the user should be told, for example that a damaged file was set aside.
    pub notice: Option<String>,
}

/// Where settings live by default: `%APPDATA%\jmeter-ui\settings.json`, or `~/.config/jmeter-ui/`.
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("jmeter-ui").join("settings.json")
}

pub fn load(path: &Path) -> Loaded {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Loaded { settings: Settings::default(), notice: None },
        Err(e) => return Loaded { settings: Settings::default(), notice: Some(format!("Could not read settings ({e}). Using defaults.")) },
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(mut s) => {
            s.sanitize();
            Loaded { settings: s, notice: None }
        }
        Err(e) => {
            let bak = path.with_extension("json.bak");
            let moved = std::fs::rename(path, &bak).is_ok();
            let notice = if moved {
                format!("Settings file was damaged ({e}). It was saved as {} and defaults are in use.", bak.display())
            } else {
                format!("Settings file was damaged ({e}). Defaults are in use.")
            };
            Loaded { settings: Settings::default(), notice: Some(notice) }
        }
    }
}

/// Save atomically: write a temporary file next to the target, then replace the target.
pub fn save(path: &Path, settings: &Settings) -> Result<()> {
    let mut s = settings.clone();
    s.sanitize();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&s)?).with_context(|| format!("cannot write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("cannot replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("shell-core-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn missing_file_gives_defaults_without_a_notice() {
        let d = temp_dir("missing");
        let l = load(&d.join("settings.json"));
        assert_eq!(l.settings, Settings::default());
        assert!(l.notice.is_none());
    }

    #[test]
    fn round_trip() {
        let d = temp_dir("roundtrip");
        let p = d.join("sub").join("settings.json");
        let mut s = Settings { theme: ThemeChoice::Dark, bottom_visible: true, jmeter_home: Some("C:/jm".into()), ..Settings::default() };
        s.push_recent("C:/plans/a.jmx");
        save(&p, &s).unwrap();
        assert!(!p.with_extension("json.tmp").exists(), "temporary file is cleaned up");
        let l = load(&p);
        assert_eq!(l.settings, s);
        assert!(l.notice.is_none());
        // Saving again over an existing file works.
        s.theme = ThemeChoice::Light;
        save(&p, &s).unwrap();
        assert_eq!(load(&p).settings.theme, ThemeChoice::Light);
    }

    #[test]
    fn older_files_with_missing_fields_still_load() {
        let d = temp_dir("old");
        let p = d.join("settings.json");
        std::fs::write(&p, r#"{ "theme": "light", "sidebar_width": 320 }"#).unwrap();
        let l = load(&p);
        assert_eq!(l.settings.theme, ThemeChoice::Light);
        assert_eq!(l.settings.sidebar_width, 320.0);
        assert_eq!(l.settings.inspector_width, Settings::default().inspector_width);
        assert!(l.notice.is_none());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let d = temp_dir("unknown");
        let p = d.join("settings.json");
        std::fs::write(&p, r#"{ "theme": "dark", "from_the_future": [1,2,3] }"#).unwrap();
        assert_eq!(load(&p).settings.theme, ThemeChoice::Dark);
    }

    #[test]
    fn damaged_file_is_set_aside() {
        let d = temp_dir("damaged");
        let p = d.join("settings.json");
        std::fs::write(&p, "{ not json").unwrap();
        let l = load(&p);
        assert_eq!(l.settings, Settings::default());
        let notice = l.notice.expect("a notice");
        assert!(notice.contains("damaged") && notice.contains(".bak"), "{notice}");
        assert!(d.join("settings.json.bak").exists());
        assert!(!p.exists());
    }

    #[test]
    fn wrong_types_count_as_damage() {
        let d = temp_dir("types");
        let p = d.join("settings.json");
        std::fs::write(&p, r#"{ "theme": 5 }"#).unwrap();
        assert!(load(&p).notice.is_some());
    }

    #[test]
    fn sizes_are_clamped_on_load_and_save() {
        let d = temp_dir("clamp");
        let p = d.join("settings.json");
        std::fs::write(&p, r#"{ "sidebar_width": 5, "inspector_width": 99999, "bottom_height": -3, "window": { "width": 10, "height": 20 } }"#).unwrap();
        let s = load(&p).settings;
        assert_eq!(s.sidebar_width, SIDEBAR_RANGE.0);
        assert_eq!(s.inspector_width, INSPECTOR_RANGE.1);
        assert_eq!(s.bottom_height, BOTTOM_RANGE.0);
        assert_eq!((s.window.width, s.window.height), WINDOW_MIN);
        let mut bad = Settings::default();
        bad.sidebar_width = f32::NAN;
        bad.sanitize();
        assert_eq!(bad.sidebar_width, Settings::default().sidebar_width);
    }

    #[test]
    fn recent_files_are_unique_ordered_and_capped() {
        let mut s = Settings::default();
        for i in 0..15 {
            s.push_recent(&format!("C:/p/{i}.jmx"));
        }
        assert_eq!(s.recent_files.len(), MAX_RECENT);
        assert_eq!(s.recent_files[0], "C:/p/14.jmx");
        s.push_recent("c:\\P\\10.JMX"); // same file with different slashes and case
        assert_eq!(s.recent_files.iter().filter(|p| p.to_lowercase().ends_with("10.jmx")).count(), 1);
        assert_eq!(s.recent_files[0], "c:\\P\\10.JMX");
        s.remove_recent("C:/p/14.jmx");
        assert!(!s.recent_files.iter().any(|p| p.ends_with("14.jmx")));
    }

    #[test]
    fn default_path_ends_in_the_app_folder() {
        let p = default_path();
        assert!(p.ends_with("jmeter-ui/settings.json") || p.ends_with("jmeter-ui\\settings.json"), "{}", p.display());
    }
}
