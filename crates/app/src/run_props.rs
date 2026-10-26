//! Properties for the runs of a plan (`name=value` lines given to JMeter as `-J` options), and the Browse button that
//! fills in a file name in a form.

use crate::shell::{Dialog, Severity, Shell};
use crate::textarea::TextArea;
use gpui::{AppContext as _, Context, PathPromptOptions, Window};
use jvm_runtime::props;
use std::path::Path;

/// The largest `.properties` file that is read into the dialog.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;

/// Settings that name a file to read, by element tag: a Browse button is shown next to them.
pub fn browse_target(tag: &str, prop: &str) -> bool {
    matches!((tag, prop), ("CSVDataSet", "filename") | ("IncludeController", "IncludeController.includepath") | ("JSR223Sampler" | "JSR223PreProcessor" | "JSR223PostProcessor" | "JSR223Assertion" | "JSR223Timer" | "JSR223Listener" | "BeanShellSampler", "filename"))
}

/// How a chosen file is written in the plan: relative to the plan's folder (with `/`, which JMeter reads on every system)
/// when it is inside it, otherwise the full path.
pub fn file_text(chosen: &Path, plan_dir: &Path) -> String {
    let (c, d) = (jvm_runtime::run::strip_verbatim(chosen), jvm_runtime::run::strip_verbatim(plan_dir));
    let same = |a: &str, b: &str| if cfg!(windows) { a.eq_ignore_ascii_case(b) } else { a == b };
    let comps: Vec<_> = c.components().collect();
    let base: Vec<_> = d.components().collect();
    if !base.is_empty() && comps.len() > base.len() && comps.iter().zip(&base).all(|(a, b)| same(&a.as_os_str().to_string_lossy(), &b.as_os_str().to_string_lossy())) {
        return comps[base.len()..].iter().map(|x| x.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    }
    c.to_string_lossy().into_owned()
}

impl Shell {
    // ---- Browse ----

    /// Ask for a file and put its name in the setting `prop` of the open form.
    pub fn browse_for_file(&mut self, prop: String, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Choose a file".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| this.set_file_field(&prop, &path, window, cx)).ok();
            }
        })
        .detach();
    }

    /// Write `path` into the setting `prop` of the open form, relative to the plan's folder when it is inside it.
    pub fn set_file_field(&mut self, prop: &str, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        // Only the settings that name a file to read take a chosen file.
        let allowed = self.form.as_ref().is_some_and(|f| self.plan().and_then(|lp| lp.plan.node(f.node)).is_some_and(|n| browse_target(&n.tag, prop)));
        if !allowed {
            return;
        }
        let plan_dir = self.plan().and_then(|lp| lp.path.parent().map(Path::to_path_buf)).unwrap_or_default();
        let text = file_text(path, &plan_dir);
        let Some(field) = self.form.as_ref().and_then(|f| {
            f.sections.iter().flat_map(|s| s.items.iter()).find_map(|i| match &i.widget {
                crate::form::Widget::Text(t) if i.def.prop == prop => Some(t.clone()),
                _ => None,
            })
        }) else {
            return;
        };
        field.update(cx, |f, cx| f.set_text(&text, cx));
        self.flush_form(window, cx);
        cx.notify();
    }

    // ---- run properties ----

    fn plan_key(&self) -> Option<String> {
        self.plan().map(|lp| crate::run_ctl::plain_path(&lp.path).to_string_lossy().into_owned())
    }

    /// Open the dialog with the properties kept for the open plan.
    pub fn open_run_props_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.plan_key() else { return };
        let text = self.settings.run_properties_for(&key).unwrap_or_default().to_string();
        let area = cx.new(|cx| TextArea::new(cx, "props-text", "host=shop.example.com\nusers=50\n# Read in the plan with ${__P(host)}", &text).rows(10));
        area.update(cx, |a, cx| a.focus(window, cx));
        self.close_dialog_on_escape(&area, window, cx);
        self.dialog = Some(Dialog::RunProps { area, error: None });
        cx.notify();
    }

    /// "Save": check the lines and keep them for this plan. A problem stays in the dialog.
    pub fn dialog_props_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::RunProps { area, .. }) = self.dialog.clone() else { return };
        let Some(key) = self.plan_key() else { return };
        let text = area.read(cx).text().to_string();
        match props::parse(&text) {
            Ok(list) => {
                self.settings.set_run_properties(&key, &text);
                self.save_settings();
                self.dialog = None;
                let n = list.len();
                self.notify(if n == 0 { "Run properties cleared".to_string() } else { format!("Saved {n} run propert{}", if n == 1 { "y" } else { "ies" }) }, Severity::Info, window, cx);
            }
            Err(e) => self.dialog = Some(Dialog::RunProps { area, error: Some(e) }),
        }
        cx.notify();
    }

    /// "Load from file": choose a `.properties` file whose lines replace the text.
    pub fn dialog_props_load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Load properties".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| this.props_from_file(&path, cx)).ok();
            }
        })
        .detach();
    }

    /// Put the lines of a `.properties` file in the dialog. A file that is too large or not text is refused in the dialog.
    pub fn props_from_file(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(Dialog::RunProps { area, .. }) = self.dialog.clone() else { return };
        let read = || -> Result<String, String> {
            let len = std::fs::metadata(path).map_err(|e| format!("{} cannot be read: {e}", path.display()))?.len();
            if len > MAX_FILE_BYTES {
                return Err(format!("{} is larger than {} KB.", path.display(), MAX_FILE_BYTES / 1024));
            }
            let bytes = std::fs::read(path).map_err(|e| format!("{} cannot be read: {e}", path.display()))?;
            String::from_utf8(bytes).map_err(|_| format!("{} is not text.", path.display()))
        };
        match read() {
            Ok(text) => {
                let text = text.trim_start_matches('\u{feff}').to_string();
                // Only the lines that are properties are brought in, so a file with other settings can still be used.
                area.update(cx, |a, cx| a.set_text(&text, cx));
                self.dialog = Some(Dialog::RunProps { area, error: props::parse(&text).err() });
            }
            Err(e) => self.dialog = Some(Dialog::RunProps { area, error: Some(e) }),
        }
        cx.notify();
    }

    /// The properties kept for the open plan, ready to give to JMeter. An error is a sentence for the person.
    pub fn run_properties(&self, plan: &Path) -> Result<Vec<(String, String)>, String> {
        let key = crate::run_ctl::plain_path(plan).to_string_lossy().into_owned();
        match self.settings.run_properties_for(&key) {
            Some(text) => props::parse(text).map_err(|e| format!("The run properties for this plan are not valid ({e}) Fix them in Run > Run Properties.")),
            None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_inside_the_plans_folder_is_written_relative_with_slashes() {
        let dir = Path::new("C:/work/shop");
        assert_eq!(file_text(Path::new("C:/work/shop/users.csv"), dir), "users.csv");
        assert_eq!(file_text(Path::new("C:/work/shop/data/2026/users.csv"), dir), "data/2026/users.csv");
        assert_eq!(file_text(Path::new("C:/work/other/users.csv"), dir), "C:/work/other/users.csv");
        assert_eq!(file_text(Path::new("C:/work/shop-old/users.csv"), dir), "C:/work/shop-old/users.csv", "a folder that only starts the same is not inside");
        assert_eq!(file_text(Path::new("C:/work/shop"), dir), "C:/work/shop", "the folder itself is not a file inside it");
        assert_eq!(file_text(Path::new("C:/a.csv"), Path::new("")), "C:/a.csv");
        if cfg!(windows) {
            assert_eq!(file_text(Path::new("c:/WORK/Shop/Users.csv"), dir), "Users.csv", "Windows ignores case");
        }
    }

    #[test]
    fn only_settings_that_name_a_file_to_read_get_a_browse_button() {
        assert!(browse_target("CSVDataSet", "filename"));
        assert!(browse_target("IncludeController", "IncludeController.includepath"));
        assert!(browse_target("JSR223Sampler", "filename"));
        assert!(!browse_target("ResultCollector", "filename"), "that one is a file to write");
        assert!(!browse_target("HTTPSamplerProxy", "HTTPSampler.path"));
        assert!(!browse_target("CSVDataSet", "variableNames"));
    }
}
