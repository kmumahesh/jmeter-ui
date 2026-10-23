//! Icons and fonts compiled into the executable.

use anyhow::Result;
use gpui::{App, AssetSource, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "../../assets"]
#[include = "icons/**/*.svg"]
#[include = "fonts/**/*.ttf"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|f| f.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter().filter(|p| p.starts_with(path)).map(|p| SharedString::from(p.to_string())).collect())
    }
}

/// Register the bundled fonts: IBM Plex Sans for the interface and Lilex for code and values.
pub fn load_fonts(cx: &App) -> Result<()> {
    let mut fonts: Vec<Cow<'static, [u8]>> = Vec::new();
    for name in Assets::iter().filter(|p| p.starts_with("fonts/")) {
        if let Some(f) = Assets::get(&name) {
            fonts.push(f.data);
        }
    }
    cx.text_system().add_fonts(fonts)?;
    Ok(())
}

pub const UI_FONT: &str = "IBM Plex Sans";
pub const MONO_FONT: &str = "Lilex";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_icon_is_bundled() {
        for c in plan_core::Category::ALL {
            let data = Assets.load(c.icon()).unwrap();
            assert!(data.is_some(), "{} is missing from the assets folder", c.icon());
        }
    }

    #[test]
    fn fonts_are_bundled() {
        let names: Vec<_> = Assets::iter().filter(|p| p.starts_with("fonts/")).collect();
        assert!(names.iter().any(|n| n.contains("IBMPlexSans-Regular")));
        assert!(names.iter().any(|n| n.contains("Lilex-Regular")));
    }

    #[test]
    fn icons_used_by_the_ui_exist() {
        for p in [
            "icons/chevron_right.svg",
            "icons/chevron_down.svg",
            "icons/close.svg",
            "icons/plus.svg",
            "icons/magnifying_glass.svg",
            "icons/play_filled.svg",
            "icons/stop.svg",
            "icons/settings.svg",
            "icons/warning.svg",
            "icons/generic_minimize.svg",
            "icons/generic_maximize.svg",
            "icons/generic_restore.svg",
            "icons/generic_close.svg",
            "icons/jm/dot.svg",
        ] {
            assert!(Assets.load(p).unwrap().is_some(), "{p}");
        }
    }

    #[test]
    fn missing_assets_return_none() {
        assert!(Assets.load("icons/nope.svg").unwrap().is_none());
        assert!(!Assets.list("icons/jm/").unwrap().is_empty());
    }
}
