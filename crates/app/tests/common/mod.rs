//! Shared helpers for driving the whole window in headless tests.

#![allow(dead_code)]

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use jmeter_ui::shell::{Options, Shell};
use jmeter_ui::{fields, shell, theme};
use shell_core::settings::{self, Settings, ThemeChoice};
use std::path::{Path, PathBuf};

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../plan-core/tests/fixtures").join(name)
}

pub const WEB: &str = "build-adv-web-test-plan.jmx";

/// The shortcut for text editing (select all, copy, undo, word moves) as the window binds it: the app uses the
/// platform key, which is Ctrl on Windows and Linux and Cmd on macOS. Write it with `ctrl-` here.
pub fn secondary(keys: &str) -> String {
    if cfg!(target_os = "macos") { keys.replace("ctrl-", "cmd-") } else { keys.to_string() }
}

pub fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("jmeter-ui-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

pub struct App<'a> {
    pub shell: Entity<Shell>,
    pub cx: &'a mut VisualTestContext,
}

pub fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        theme::init(cx, ThemeChoice::Dark);
        fields::bind_keys(cx);
        shell::bind_keys(cx);
    });
}

pub fn start<'a>(cx: &'a mut TestAppContext, open: Option<PathBuf>, settings_path: Option<PathBuf>) -> App<'a> {
    setup(cx);
    let loaded = settings_path.as_ref().map(|p| settings::load(p));
    let (settings, notice) = match loaded {
        Some(l) => (l.settings, l.notice),
        None => (Settings::default(), None),
    };
    let (shell, cx) = cx.add_window_view(move |window, cx| Shell::new(window, cx, Options { settings, notice, settings_path, open, no_external_open: true, ..Default::default() }));
    // Focus changes are only reported while the window counts as active, as it does on screen.
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    App { shell, cx }
}

impl App<'_> {
    pub fn state(&mut self) -> serde_json::Value {
        let shell = self.shell.clone();
        self.cx.update(|_, cx| shell.read(cx).state_json(cx))
    }

    pub fn keys(&mut self, k: &str) {
        // The tests write the Windows keys; a Mac has Command where they have Ctrl (except for tabs).
        let k = if cfg!(target_os = "macos") {
            k.split(' ').map(|part| if part.starts_with("ctrl-") && !part.ends_with("tab") { part.replacen("ctrl-", "cmd-", 1) } else { part.to_string() }).collect::<Vec<_>>().join(" ")
        } else {
            k.to_string()
        };
        self.cx.simulate_keystrokes(&k);
        self.cx.run_until_parked();
    }

    pub fn input(&mut self, s: &str) {
        self.cx.simulate_input(s);
        self.cx.run_until_parked();
    }

    pub fn click(&mut self, selector: &'static str) {
        self.settle();
        let b = self.cx.debug_bounds(selector).unwrap_or_else(|| panic!("no element with selector {selector}"));
        self.cx.simulate_click(b.center(), Modifiers::default());
        self.cx.run_until_parked();
    }

    pub fn has(&mut self, selector: &'static str) -> bool {
        self.settle();
        self.cx.debug_bounds(selector).is_some()
    }

    /// Draw the window again so what was just changed has been laid out before it is measured.
    pub fn settle(&mut self) {
        self.cx.update(|window, _| window.refresh());
        self.cx.run_until_parked();
    }

    pub fn tabs(&mut self) -> Vec<String> {
        let s = self.state();
        s["tabs"].as_array().unwrap().iter().map(|t| t["title"].as_str().unwrap().to_string()).collect()
    }

    pub fn active_tab(&mut self) -> Option<String> {
        self.state()["active_tab"].as_str().map(str::to_string)
    }

    /// The name of the element the editor is showing.
    pub fn editing(&mut self) -> Option<String> {
        self.state()["editor"]["name"].as_str().map(str::to_string)
    }

    pub fn open_path(&mut self, path: &Path) {
        let shell = self.shell.clone();
        let path = path.to_path_buf();
        self.cx.update(|window, cx| {
            shell.update(cx, |s, cx| {
                s.open_plan_path(&path, window, cx);
            })
        });
        self.cx.run_until_parked();
    }

    pub fn selected(&mut self) -> Option<String> {
        self.state()["selected"].as_str().map(str::to_string)
    }

    pub fn palette_titles(&mut self) -> Vec<String> {
        self.state()["palette"]["results"].as_array().map(|a| a.iter().map(|t| t.as_str().unwrap().to_string()).collect()).unwrap_or_default()
    }

    /// Palette results that are shown but cannot be chosen.
    pub fn palette_unavailable(&mut self) -> Vec<String> {
        self.state()["palette"]["unavailable"].as_array().map(|a| a.iter().map(|t| t.as_str().unwrap().to_string()).collect()).unwrap_or_default()
    }

    /// Turn the mouse wheel over the middle of the window, so that `dy` pixels more of a scrolling page show.
    pub fn scroll_down(&mut self, dy: f32) {
        let at = gpui::point(gpui::px(640.), gpui::px(400.));
        self.cx.simulate_event(gpui::ScrollWheelEvent {
            position: at,
            delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.), gpui::px(-dy))),
            modifiers: gpui::Modifiers::default(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        self.cx.run_until_parked();
    }

    pub fn palette_open(&mut self) -> bool {
        !self.state()["palette"].is_null()
    }
}


impl App<'_> {
    /// A saved setting of the selected element (or of the test plan when nothing is selected).
    pub fn prop(&mut self, name: &str) -> Option<String> {
        let shell = self.shell.clone();
        let name = name.to_string();
        self.cx.update(|_, cx| {
            let s = shell.read(cx);
            let lp = s.plan()?;
            let id = lp.tree.selected().or_else(|| lp.plan.roots().first().copied())?;
            lp.edit.prop(id, &name)
        })
    }

    /// Click into a box, replace what is in it with `text` and press Enter.
    pub fn type_into(&mut self, selector: &'static str, text: &str) {
        self.click(selector);
        self.keys(&secondary("ctrl-a"));
        self.input(text);
        self.keys("enter");
    }

    /// The drop-down for a setting in the open form.
    pub fn combo(&mut self, prop: &str) -> Entity<jmeter_ui::combo::ComboBox> {
        let shell = self.shell.clone();
        let prop = prop.to_string();
        self.cx.update(|_, cx| {
            let s = shell.read(cx);
            let form = s.form.as_ref().expect("a form is open");
            form.sections
                .iter()
                .flat_map(|sec| sec.items.iter())
                .find_map(|i| match (&i.widget, i.def.prop == prop) {
                    (jmeter_ui::form::Widget::Combo(c), true) => Some(c.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no drop-down for {prop}"))
        })
    }

    pub fn choose(&mut self, prop: &str, index: usize) {
        let c = self.combo(prop);
        self.cx.update(|_, cx| c.update(cx, |c, cx| c.select(index, cx)));
        self.cx.run_until_parked();
    }

    /// The text of the plan file's element, for checking what would be saved.
    pub fn plan_text(&mut self) -> String {
        let shell = self.shell.clone();
        self.cx.update(|_, cx| shell.read(cx).plan().unwrap().edit.to_text())
    }
}

impl App<'_> {
    /// Press on one element, move to another and let go.
    pub fn drag(&mut self, from: &'static str, to: &'static str) {
        let a = self.cx.debug_bounds(from).unwrap_or_else(|| panic!("no element {from}")).center();
        let b = self.cx.debug_bounds(to).unwrap_or_else(|| panic!("no element {to}")).center();
        let mods = gpui::Modifiers::default();
        self.cx.simulate_mouse_down(a, gpui::MouseButton::Left, mods);
        let mid = gpui::point((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        self.cx.simulate_mouse_move(mid, Some(gpui::MouseButton::Left), mods);
        self.cx.simulate_mouse_move(b, Some(gpui::MouseButton::Left), mods);
        self.cx.run_until_parked();
        self.cx.simulate_mouse_up(b, gpui::MouseButton::Left, mods);
        self.cx.run_until_parked();
    }

    /// The names of an element's children, in order.
    pub fn children_of(&mut self, name: &str) -> Vec<String> {
        let shell = self.shell.clone();
        let name = name.to_string();
        self.cx.update(|_, cx| {
            let s = shell.read(cx);
            let p = &s.plan().unwrap().plan;
            let id = p.find_by_name(&name).unwrap_or_else(|| panic!("no element {name}"));
            p.node(id).unwrap().children.iter().map(|&c| p.node(c).unwrap().name.clone()).collect()
        })
    }
}
