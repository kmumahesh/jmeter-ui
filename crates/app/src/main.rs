use gpui::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;
use jmeter_ui::assets::{self, Assets};
use jmeter_ui::shell::{self, Options, Shell};
use jmeter_ui::{fields, theme};
use shell_core::settings::{self, ThemeChoice, WINDOW_MIN};
use std::path::PathBuf;

/// Command line: `jmeter-ui [plan.jmx] [--open plan.jmx] [--settings file] [--theme dark|light|system] [--state file] [--no-detect]`
struct Args {
    open: Option<PathBuf>,
    settings: Option<PathBuf>,
    state: Option<PathBuf>,
    theme: Option<ThemeChoice>,
    detect: bool,
}

fn parse_args() -> Args {
    let mut a = Args { open: None, settings: None, state: None, theme: None, detect: true };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--open" => a.open = it.next().map(PathBuf::from),
            "--settings" => a.settings = it.next().map(PathBuf::from),
            "--state" => a.state = it.next().map(PathBuf::from),
            "--theme" => {
                a.theme = it.next().and_then(|t| match t.as_str() {
                    "dark" => Some(ThemeChoice::Dark),
                    "light" => Some(ThemeChoice::Light),
                    "system" => Some(ThemeChoice::System),
                    _ => None,
                })
            }
            "--no-detect" => a.detect = false,
            other if !other.starts_with("--") => a.open = Some(PathBuf::from(other)),
            _ => {}
        }
    }
    a
}

fn main() {
    let args = parse_args();
    let settings_path = args.settings.clone().unwrap_or_else(settings::default_path);
    let loaded = settings::load(&settings_path);
    application().with_assets(Assets).run(move |cx: &mut App| {
        if let Err(e) = assets::load_fonts(cx) {
            eprintln!("could not load fonts: {e:#}");
        }
        let mut s = loaded.settings.clone();
        if let Some(t) = args.theme {
            s.theme = t;
        }
        theme::init(cx, s.theme);
        fields::bind_keys(cx);
        shell::bind_keys(cx);
        let bounds = Bounds::centered(None, size(px(s.window.width), px(s.window.height)), cx);
        let options = Options {
            settings: loaded.settings.clone(),
            notice: loaded.notice.clone(),
            settings_path: Some(settings_path.clone()),
            open: args.open.clone(),
            state_dump: args.state.clone(),
            theme: args.theme,
            detect_runtime: args.detect,
        };
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(if s.window.maximized { WindowBounds::Maximized(bounds) } else { WindowBounds::Windowed(bounds) }),
                    titlebar: Some(TitlebarOptions { title: Some("JMeter UI".into()), appears_transparent: true, traffic_light_position: None }),
                    window_min_size: Some(size(px(WINDOW_MIN.0), px(WINDOW_MIN.1))),
                    ..Default::default()
                },
                move |window, cx| {
                    let shell = cx.new(|cx| Shell::new(window, cx, options));
                    let handle = shell.clone();
                    window.on_window_should_close(cx, move |window, cx| {
                        handle.update(cx, |s, cx| {
                            s.flush_form(window, cx);
                            let ok = s.request_quit(cx);
                            if ok {
                                s.save_window_state(window);
                            }
                            ok
                        })
                    });
                    shell
                },
            )
            .expect("could not open the window");
        let _ = window;
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
