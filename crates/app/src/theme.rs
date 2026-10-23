//! Colours and fonts. A small palette per mode is mapped onto Zed's theme slots, so Zed's own
//! components (buttons, tabs, list items, menus) pick up our look.

use crate::assets::{MONO_FONT, UI_FONT};
use gpui::{App, BorrowAppContext, Font, FontFeatures, FontStyle, FontWeight, Global, Hsla, Pixels, WindowAppearance, px, rgb};
use plan_core::Category;
use shell_core::settings::ThemeChoice;
use std::sync::Arc;
use theme::{ActiveTheme, GlobalTheme, Theme, ThemeRegistry, UiDensity};

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub bg: Hsla,
    pub panel: Hsla,
    pub panel2: Hsla,
    pub raise: Hsla,
    pub line: Hsla,
    pub fg: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub ok: Hsla,
    pub warn: Hsla,
    pub err: Hsla,
    /// Text on top of the accent colour (the status bar).
    pub on_accent: Hsla,
    categories: [Hsla; 14],
}

fn hex(v: u32) -> Hsla {
    rgb(v).into()
}

pub fn with_alpha(c: Hsla, a: f32) -> Hsla {
    Hsla { a, ..c }
}

impl Palette {
    pub fn dark() -> Palette {
        Palette {
            dark: true,
            bg: hex(0x16181d),
            panel: hex(0x1c1f26),
            panel2: hex(0x232730),
            raise: hex(0x262a33),
            line: hex(0x2e333d),
            fg: hex(0xd6dae2),
            muted: hex(0x8d95a5),
            accent: hex(0x6f9fff),
            ok: hex(0x7fd28a),
            warn: hex(0xe5c07b),
            err: hex(0xf07a84),
            on_accent: hex(0x0b1220),
            // Order follows Category::ALL.
            categories: [
                hex(0xa99bff), // test plan
                hex(0x6f9fff), // thread group
                hex(0x58b4f0), // sampler
                hex(0xe5b45b), // controller
                hex(0x4fd1c0), // config
                hex(0xc59bf0), // timer
                hex(0xf09a62), // pre-processor
                hex(0xf27fb8), // post-processor
                hex(0x7fd28a), // assertion
                hex(0x9aa3b3), // listener
                hex(0xa99bff), // fragment
                hex(0x9aa3b3), // workbench
                hex(0x9aa3b3), // non-test
                hex(0x8d95a5), // unknown
            ],
        }
    }

    pub fn light() -> Palette {
        Palette {
            dark: false,
            bg: hex(0xf6f7f9),
            panel: hex(0xeceef2),
            panel2: hex(0xe3e6ec),
            raise: hex(0xffffff),
            line: hex(0xd5d9e1),
            fg: hex(0x1f2430),
            muted: hex(0x5d6676),
            accent: hex(0x2a64d8),
            ok: hex(0x1e8a45),
            warn: hex(0xa96a00),
            err: hex(0xc8323e),
            on_accent: hex(0xffffff),
            categories: [
                hex(0x6d5bd0),
                hex(0x2a64d8),
                hex(0x1b7fc4),
                hex(0xb7791f),
                hex(0x0f8a7e),
                hex(0x8a4fc0),
                hex(0xc0602a),
                hex(0xc4488a),
                hex(0x1e8a45),
                hex(0x5d6676),
                hex(0x6d5bd0),
                hex(0x5d6676),
                hex(0x5d6676),
                hex(0x5d6676),
            ],
        }
    }

    pub fn for_mode(dark: bool) -> Palette {
        if dark { Palette::dark() } else { Palette::light() }
    }

    pub fn category(&self, c: Category) -> Hsla {
        let i = Category::ALL.iter().position(|&x| x == c).unwrap_or(Category::ALL.len() - 1);
        self.categories[i]
    }

    /// A translucent accent for selections and highlights.
    pub fn selection(&self) -> Hsla {
        with_alpha(self.accent, if self.dark { 0.16 } else { 0.14 })
    }
}

/// WCAG relative luminance of an opaque colour.
pub fn luminance(c: Hsla) -> f32 {
    let rgba = c.to_rgb();
    let lin = |v: f32| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * lin(rgba.r) + 0.7152 * lin(rgba.g) + 0.0722 * lin(rgba.b)
}

/// WCAG contrast ratio between two opaque colours, from 1 to 21.
pub fn contrast(a: Hsla, b: Hsla) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

// ---- global state ----

struct ThemeState {
    base: Arc<Theme>,
    choice: ThemeChoice,
    palette: Palette,
}

impl Global for ThemeState {}

/// The palette currently in use.
pub fn palette(cx: &App) -> Palette {
    cx.global::<ThemeState>().palette
}

pub fn choice(cx: &App) -> ThemeChoice {
    cx.global::<ThemeState>().choice
}

struct Provider {
    ui: Font,
    buffer: Font,
}

impl theme::ThemeSettingsProvider for Provider {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.ui
    }
    fn buffer_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.buffer
    }
    fn ui_font_size(&self, _cx: &App) -> Pixels {
        px(13.)
    }
    fn buffer_font_size(&self, _cx: &App) -> Pixels {
        px(12.5)
    }
    fn ui_density(&self, _cx: &App) -> UiDensity {
        UiDensity::Default
    }
}

fn font(family: &'static str) -> Font {
    Font { family: family.into(), features: FontFeatures::default(), fallbacks: None, weight: FontWeight::NORMAL, style: FontStyle::Normal }
}

fn system_is_dark(cx: &App) -> bool {
    matches!(cx.window_appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark)
}

/// Set up Zed's theme system with our fonts and the chosen mode. Call once at startup.
pub fn init(cx: &mut App, choice: ThemeChoice) {
    theme::set_theme_settings_provider(Box::new(Provider { ui: font(UI_FONT), buffer: font(MONO_FONT) }), cx);
    theme::init(theme::LoadThemes::JustBase, cx);
    let base = ThemeRegistry::global(cx).get(theme::DEFAULT_DARK_THEME).expect("the built-in dark theme exists");
    cx.set_global(ThemeState { base, choice, palette: Palette::dark() });
    apply(cx, choice);
}

/// Switch mode. `System` follows the operating system.
pub fn apply(cx: &mut App, choice: ThemeChoice) {
    let dark = match choice {
        ThemeChoice::Dark => true,
        ThemeChoice::Light => false,
        ThemeChoice::System => system_is_dark(cx),
    };
    let palette = Palette::for_mode(dark);
    let base = cx.global::<ThemeState>().base.clone();
    let theme = Arc::new(build_theme(&base, &palette));
    cx.update_global::<ThemeState, _>(|s, _| {
        s.choice = choice;
        s.palette = palette;
    });
    GlobalTheme::update_theme(cx, theme);
}

/// Re-evaluate `System` after the operating system changes mode.
pub fn system_appearance_changed(cx: &mut App) {
    let choice = choice(cx);
    if choice == ThemeChoice::System {
        apply(cx, choice);
    }
}

/// Our palette laid over a base theme's slots.
pub fn build_theme(base: &Theme, p: &Palette) -> Theme {
    let mut t = base.clone();
    t.id = if p.dark { "jmeter-dark".into() } else { "jmeter-light".into() };
    t.name = if p.dark { "JMeter Dark".into() } else { "JMeter Light".into() };
    t.appearance = if p.dark { theme::Appearance::Dark } else { theme::Appearance::Light };
    let none = with_alpha(p.bg, 0.0);
    let c = &mut t.styles.colors;
    c.border = p.line;
    c.border_variant = with_alpha(p.line, 0.7);
    c.border_focused = p.accent;
    c.border_selected = with_alpha(p.accent, 0.5);
    c.border_transparent = none;
    c.border_disabled = p.line;
    c.elevated_surface_background = p.raise;
    c.surface_background = p.panel;
    c.background = p.bg;
    c.element_background = p.panel2;
    c.element_hover = p.raise;
    c.element_active = p.line;
    c.element_selected = p.selection();
    c.element_selection_background = with_alpha(p.accent, 0.25);
    c.element_disabled = p.panel;
    c.drop_target_background = with_alpha(p.accent, 0.2);
    c.drop_target_border = p.accent;
    c.ghost_element_background = none;
    c.ghost_element_hover = p.panel2;
    c.ghost_element_active = p.line;
    c.ghost_element_selected = p.selection();
    c.ghost_element_disabled = none;
    c.text = p.fg;
    c.text_muted = p.muted;
    c.text_placeholder = with_alpha(p.muted, 0.8);
    c.text_disabled = with_alpha(p.muted, 0.6);
    c.text_accent = p.accent;
    c.icon = p.fg;
    c.icon_muted = p.muted;
    c.icon_disabled = with_alpha(p.muted, 0.6);
    c.icon_placeholder = p.muted;
    c.icon_accent = p.accent;
    c.status_bar_background = p.accent;
    c.title_bar_background = p.panel;
    c.title_bar_inactive_background = p.panel;
    c.toolbar_background = p.bg;
    c.tab_bar_background = p.panel;
    c.tab_inactive_background = p.panel;
    c.tab_active_background = p.bg;
    c.search_match_background = with_alpha(p.accent, 0.3);
    c.panel_background = p.panel;
    c.panel_focused_border = p.accent;
    c.panel_indent_guide = with_alpha(p.line, 0.8);
    c.panel_indent_guide_hover = p.muted;
    c.panel_indent_guide_active = with_alpha(p.accent, 0.6);
    c.pane_focused_border = p.accent;
    c.pane_group_border = p.line;
    c.scrollbar_thumb_background = with_alpha(p.muted, 0.3);
    c.scrollbar_thumb_hover_background = with_alpha(p.muted, 0.5);
    c.scrollbar_thumb_active_background = with_alpha(p.muted, 0.7);
    c.scrollbar_thumb_border = none;
    c.scrollbar_track_background = none;
    c.scrollbar_track_border = p.line;
    c.editor_background = p.bg;
    c.editor_foreground = p.fg;
    c.editor_gutter_background = p.bg;
    c.editor_line_number = p.muted;
    c.editor_active_line_number = p.fg;
    c.editor_active_line_background = p.selection();
    c.link_text_hover = p.accent;
    let s = &mut t.styles.status;
    s.error = p.err;
    s.error_background = with_alpha(p.err, 0.15);
    s.error_border = with_alpha(p.err, 0.4);
    s.warning = p.warn;
    s.warning_background = with_alpha(p.warn, 0.15);
    s.warning_border = with_alpha(p.warn, 0.4);
    s.success = p.ok;
    s.success_background = with_alpha(p.ok, 0.15);
    s.success_border = with_alpha(p.ok, 0.4);
    s.info = p.accent;
    s.info_background = with_alpha(p.accent, 0.15);
    s.info_border = with_alpha(p.accent, 0.4);
    t
}

/// Convenience for views: the active Zed colours.
pub fn colors(cx: &App) -> &theme::ThemeColors {
    cx.theme().colors()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_math_matches_known_values() {
        let black = hex(0x000000);
        let white = hex(0xffffff);
        assert!((contrast(black, white) - 21.0).abs() < 0.01);
        assert!((contrast(white, white) - 1.0).abs() < 0.001);
        assert!((contrast(hex(0x777777), white) - 4.48).abs() < 0.05);
    }

    #[test]
    fn text_is_readable_in_both_modes() {
        for p in [Palette::dark(), Palette::light()] {
            let mode = if p.dark { "dark" } else { "light" };
            for (name, surface) in [("bg", p.bg), ("panel", p.panel), ("panel2", p.panel2)] {
                assert!(contrast(p.fg, surface) >= 7.0, "{mode}: text on {name} is {:.2}", contrast(p.fg, surface));
                assert!(contrast(p.muted, surface) >= 4.5, "{mode}: muted text on {name} is {:.2}", contrast(p.muted, surface));
            }
            assert!(contrast(p.on_accent, p.accent) >= 4.5, "{mode}: status bar text {:.2}", contrast(p.on_accent, p.accent));
        }
    }

    #[test]
    fn colours_that_carry_meaning_stand_out_from_the_background() {
        for p in [Palette::dark(), Palette::light()] {
            let mode = if p.dark { "dark" } else { "light" };
            for (name, c) in [("accent", p.accent), ("ok", p.ok), ("warn", p.warn), ("err", p.err)] {
                assert!(contrast(c, p.bg) >= 3.0, "{mode}: {name} on bg is {:.2}", contrast(c, p.bg));
            }
            for cat in Category::ALL {
                if cat == Category::Unknown {
                    continue;
                }
                assert!(contrast(p.category(cat), p.panel) >= 3.0, "{mode}: {cat:?} icon on panel is {:.2}", contrast(p.category(cat), p.panel));
            }
        }
    }

    #[test]
    fn dark_is_dark_and_light_is_light() {
        assert!(luminance(Palette::dark().bg) < 0.05);
        assert!(luminance(Palette::light().bg) > 0.8);
        assert!(luminance(Palette::dark().fg) > luminance(Palette::dark().bg));
        assert!(luminance(Palette::light().fg) < luminance(Palette::light().bg));
    }

    #[test]
    fn surfaces_are_distinguishable_from_each_other() {
        for p in [Palette::dark(), Palette::light()] {
            assert!(contrast(p.bg, p.panel) > 1.03);
            assert!(contrast(p.panel, p.line) > 1.05);
        }
    }

    #[test]
    fn every_category_has_a_colour() {
        let p = Palette::dark();
        let mut distinct = std::collections::HashSet::new();
        for c in Category::ALL {
            let col = p.category(c);
            distinct.insert(format!("{:.3}-{:.3}-{:.3}", col.h, col.s, col.l));
        }
        assert!(distinct.len() >= 9, "categories should be visually distinct, got {}", distinct.len());
    }

    #[test]
    fn alpha_helper_only_changes_alpha() {
        let c = with_alpha(hex(0x6f9fff), 0.25);
        assert_eq!(c.a, 0.25);
        assert_eq!(c.h, hex(0x6f9fff).h);
    }

    #[test]
    fn selection_is_translucent() {
        assert!(Palette::dark().selection().a < 0.3);
        assert!(Palette::light().selection().a < 0.3);
    }
}
