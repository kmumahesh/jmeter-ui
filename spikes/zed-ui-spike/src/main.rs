//! Spike 1b: can we depend on Zed's own `ui` and `theme` crates (GPL-3.0-or-later) directly?
use gpui::{App, AssetSource, Bounds, Context, SharedString, Window, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;
use std::borrow::Cow;
use std::path::PathBuf;
use theme::ActiveTheme;
use ui::{Button, ButtonStyle, Divider, IconButton, IconName, Label, ListItem, Tab, TabBar, Tooltip, prelude::*};

struct Assets(PathBuf);

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(std::fs::read(self.0.join(path)).ok().map(Cow::Owned))
    }
    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

struct Provider {
    ui: gpui::Font,
    buffer: gpui::Font,
}

impl theme::ThemeSettingsProvider for Provider {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a gpui::Font {
        &self.ui
    }
    fn buffer_font<'a>(&'a self, _cx: &'a App) -> &'a gpui::Font {
        &self.buffer
    }
    fn ui_font_size(&self, _cx: &App) -> gpui::Pixels {
        px(13.)
    }
    fn buffer_font_size(&self, _cx: &App) -> gpui::Pixels {
        px(13.)
    }
    fn ui_density(&self, _cx: &App) -> theme::UiDensity {
        theme::UiDensity::Default
    }
}

fn font(family: &'static str) -> gpui::Font {
    gpui::Font {
        family: family.into(),
        features: gpui::FontFeatures::default(),
        fallbacks: None,
        weight: gpui::FontWeight::NORMAL,
        style: gpui::FontStyle::Normal,
    }
}

struct View;

impl Render for View {
    fn render(&mut self, _w: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        v_flex()
            .size_full()
            .bg(colors.background)
            .text_color(colors.text)
            .child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .bg(colors.title_bar_background)
                    .child(Button::new("run", "Run").style(ButtonStyle::Filled).start_icon(Icon::new(IconName::PlayFilled)))
                    .child(IconButton::new("stop", IconName::Stop).tooltip(Tooltip::text("Stop the run")))
                    .child(IconButton::new("search", IconName::MagnifyingGlass).tooltip(Tooltip::text("Search")))
                    .child(IconButton::new("settings", IconName::Settings)),
            )
            .child(Divider::horizontal())
            .child(
                TabBar::new("tabs")
                    .child(Tab::new("t1").toggle_state(true).child(Label::new("POST /api/login")))
                    .child(Tab::new("t2").child(Label::new("Checkout Users"))),
            )
            .child(
                v_flex()
                    .p_2()
                    .gap_1()
                    .child(ListItem::new("a").child(Label::new("GET /api/products")).start_slot(Icon::new(IconName::File)))
                    .child(ListItem::new("b").toggle_state(true).child(Label::new("POST /api/checkout")).start_slot(Icon::new(IconName::File)))
                    .child(ListItem::new("c").child(Label::new("GET /api/cart").color(Color::Muted)).start_slot(Icon::new(IconName::Folder))),
            )
    }
}

fn main() {
    let icons = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    application().with_assets(Assets(icons)).run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf") as &[u8]),
                Cow::Borrowed(include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf") as &[u8]),
                Cow::Borrowed(include_bytes!("../../../assets/fonts/lilex/Lilex-Regular.ttf") as &[u8]),
            ])
            .unwrap();
        theme::set_theme_settings_provider(Box::new(Provider { ui: font("IBM Plex Sans"), buffer: font("Lilex") }), cx);
        theme::init(theme::LoadThemes::JustBase, cx);
        let bounds = Bounds::centered(None, size(px(700.), px(420.)), cx);
        cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            |_, cx| cx.new(|_| View),
        )
        .unwrap();
        cx.activate(true);
    });
}
