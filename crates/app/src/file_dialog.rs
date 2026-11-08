//! The system's Open and Save dialogs, asking for the right kind of file.
//!
//! GPUI's own file questions list every file and cannot be told what kind of file is wanted. On Windows these questions
//! are asked with the system's file dialog directly, so Open and Save As show "JMeter file (*.jmx)" by default and list
//! only those files, with "All files" as a second choice. Other systems keep GPUI's questions.

use gpui::{App, Task, Window};
use std::path::{Path, PathBuf};

/// What kind of file a question asks for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A JMeter plan (`.jmx`).
    Plan,
    /// A recording saved from a browser (`.har`).
    Har,
    /// JMeter's results (`.jtl` or `.csv`).
    Results,
    /// A properties file (`.properties`).
    Properties,
    /// A file with data for a test (`.csv`, `.txt`).
    Data,
}

/// One entry of the "type of file" list.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub label: &'static str,
    /// File name patterns separated by semicolons, for example `*.jtl;*.csv`.
    pub patterns: &'static str,
}

const ALL_FILES: Filter = Filter { label: "All files (*.*)", patterns: "*.*" };

/// The type list for a kind of file: the kind's own type first (the one shown), then "All files".
pub fn filters(kind: Kind) -> Vec<Filter> {
    let own = match kind {
        Kind::Plan => Filter { label: "JMeter file (*.jmx)", patterns: "*.jmx" },
        Kind::Har => Filter { label: "HAR file (*.har)", patterns: "*.har" },
        Kind::Results => Filter { label: "JMeter results (*.jtl, *.csv)", patterns: "*.jtl;*.csv" },
        Kind::Properties => Filter { label: "Properties file (*.properties)", patterns: "*.properties" },
        Kind::Data => Filter { label: "Data file (*.csv, *.txt)", patterns: "*.csv;*.txt" },
    };
    vec![own, ALL_FILES]
}

/// The extension a saved file of this kind gets when its name has none.
pub fn default_extension(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Plan => Some("jmx"),
        Kind::Properties => Some("properties"),
        Kind::Har | Kind::Results | Kind::Data => None,
    }
}

/// `path` with the kind's default extension added when it has none (`test` becomes `test.jmx`). A name that has an
/// extension, of any kind, is left as the user typed it.
pub fn with_default_extension(path: &Path, kind: Kind) -> PathBuf {
    let mut path = path.to_path_buf();
    if path.extension().is_none()
        && let Some(ext) = default_extension(kind)
    {
        path.set_extension(ext);
    }
    path
}

/// Answers that stand in for the real file question while a test runs: set it as a global, and each question takes the
/// next answer (`None` is "cancelled"; an empty list cancels too), so no window is opened. The questions asked are kept
/// so a test can check what was asked.
#[derive(Default)]
pub struct ScriptedAnswers {
    pub answers: std::collections::VecDeque<Option<PathBuf>>,
    pub asked: Vec<Asked>,
}

impl gpui::Global for ScriptedAnswers {}

/// A question a test answered: its kind, the folder it started in (saving only) and the name it suggested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    pub kind: Kind,
    pub dir: Option<PathBuf>,
    pub suggested: String,
}

fn scripted(cx: &mut App, asked: Asked) -> Option<Option<PathBuf>> {
    if !cx.has_global::<ScriptedAnswers>() {
        return None;
    }
    let s = cx.global_mut::<ScriptedAnswers>();
    s.asked.push(asked);
    Some(s.answers.pop_front().unwrap_or(None))
}

/// Ask for a file to open. The task gives the chosen file, or nothing when the question was cancelled.
pub fn open(window: &Window, cx: &mut App, kind: Kind, title: &str) -> Task<Option<PathBuf>> {
    if let Some(answer) = scripted(cx, Asked { kind, dir: None, suggested: String::new() }) {
        return Task::ready(answer);
    }
    #[cfg(windows)]
    {
        let owner = native::owner_of(window);
        let title = title.to_string();
        cx.background_executor().spawn(async move { native::pick(owner, kind, &title, None) })
    }
    #[cfg(not(windows))]
    {
        let _ = (window, kind);
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some(title.to_string().into()) });
        cx.spawn(async move |_| match rx.await {
            Ok(Ok(Some(paths))) => paths.into_iter().next(),
            _ => None,
        })
    }
}

/// Ask for a file name to save to, starting in `dir` with `suggested` filled in. The task gives the chosen path (with the
/// kind's extension added when the name has none), or nothing when the question was cancelled.
pub fn save(window: &Window, cx: &mut App, kind: Kind, dir: &Path, suggested: &str) -> Task<Option<PathBuf>> {
    if let Some(answer) = scripted(cx, Asked { kind, dir: Some(dir.to_path_buf()), suggested: suggested.to_string() }) {
        return Task::ready(answer.map(|p| with_default_extension(&p, kind)));
    }
    #[cfg(windows)]
    {
        let owner = native::owner_of(window);
        let (dir, suggested) = (dir.to_path_buf(), suggested.to_string());
        cx.background_executor().spawn(async move { native::pick(owner, kind, "", Some((dir, suggested))).map(|p| with_default_extension(&p, kind)) })
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        let rx = cx.prompt_for_new_path(dir, Some(suggested));
        cx.spawn(async move |_| match rx.await {
            Ok(Ok(Some(path))) => Some(with_default_extension(&path, kind)),
            _ => None,
        })
    }
}

/// Builds the system dialog for a kind of file without showing it, and says which entry of the type list is selected
/// (1 is the first). For tests: it proves the system accepts the type list.
#[cfg(windows)]
pub fn check_type_list(kind: Kind, saving: bool) -> Result<u32, String> {
    native::check_type_list(kind, saving).map_err(|e| e.to_string())
}

#[cfg(windows)]
mod native {
    use super::{Kind, filters};
    use gpui::Window;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::path::{Path, PathBuf};
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize};
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH};
    use windows::core::{HRESULT, HSTRING, PCWSTR};

    /// The window handle the question belongs to, so it blocks that window while it is open.
    pub fn owner_of(window: &Window) -> Option<isize> {
        match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
            _ => None,
        }
    }

    /// Windows' dialogs need a single-threaded COM apartment on the thread that shows them.
    struct Apartment(bool);

    impl Apartment {
        fn new() -> Self {
            // Safe to call on any thread; an apartment of another kind already there is left alone.
            Apartment(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
        }
    }

    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    fn apply_type_list(dialog: &IFileDialog, kind: Kind) -> windows::core::Result<()> {
        let list = filters(kind);
        let labels: Vec<HSTRING> = list.iter().map(|f| HSTRING::from(f.label)).collect();
        let patterns: Vec<HSTRING> = list.iter().map(|f| HSTRING::from(f.patterns)).collect();
        let specs: Vec<COMDLG_FILTERSPEC> = labels.iter().zip(&patterns).map(|(l, p)| COMDLG_FILTERSPEC { pszName: PCWSTR(l.as_ptr()), pszSpec: PCWSTR(p.as_ptr()) }).collect();
        unsafe {
            dialog.SetFileTypes(&specs)?;
            dialog.SetFileTypeIndex(1)?;
        }
        if let Some(ext) = super::default_extension(kind) {
            unsafe { dialog.SetDefaultExtension(&HSTRING::from(ext))? };
        }
        Ok(())
    }

    pub fn check_type_list(kind: Kind, saving: bool) -> windows::core::Result<u32> {
        let _apartment = Apartment::new();
        unsafe {
            if saving {
                let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL)?;
                apply_type_list(&dialog, kind)?;
                dialog.GetFileTypeIndex()
            } else {
                let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;
                apply_type_list(&dialog, kind)?;
                dialog.GetFileTypeIndex()
            }
        }
    }

    fn start_folder(dialog: &IFileDialog, dir: &Path) {
        let plain = crate::run_ctl::plain_path(&dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf()));
        if let Ok(item) = unsafe { SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(plain.display().to_string()), None) } {
            unsafe { dialog.SetFolder(&item).ok() };
        }
    }

    fn path_of(item: &IShellItem) -> windows::core::Result<PathBuf> {
        unsafe {
            let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let text = name.to_string();
            CoTaskMemFree(Some(name.0 as _));
            Ok(PathBuf::from(text.map_err(|_| windows::core::Error::from(windows::Win32::Foundation::E_UNEXPECTED))?))
        }
    }

    /// Show the question and wait. `save` is the folder and suggested name for a Save question. Nothing when the user
    /// cancels or Windows cannot show it (the error is written to the log).
    pub fn pick(owner: Option<isize>, kind: Kind, title: &str, save: Option<(PathBuf, String)>) -> Option<PathBuf> {
        let _apartment = Apartment::new();
        let owner = owner.map(|h| HWND(h as *mut core::ffi::c_void));
        let result = (|| -> windows::core::Result<Option<PathBuf>> {
            unsafe {
                if let Some((dir, suggested)) = &save {
                    let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL)?;
                    apply_type_list(&dialog, kind)?;
                    dialog.SetOptions(FOS_OVERWRITEPROMPT | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM)?;
                    start_folder(&dialog, dir);
                    dialog.SetFileName(&HSTRING::from(suggested.as_str()))?;
                    match dialog.Show(owner) {
                        Err(e) if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => return Ok(None),
                        other => other?,
                    }
                    Ok(Some(path_of(&dialog.GetResult()?)?))
                } else {
                    let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;
                    apply_type_list(&dialog, kind)?;
                    dialog.SetOptions(FOS_FILEMUSTEXIST | FOS_PATHMUSTEXIST | FOS_FORCEFILESYSTEM)?;
                    if !title.is_empty() {
                        dialog.SetTitle(&HSTRING::from(title))?;
                    }
                    match dialog.Show(owner) {
                        Err(e) if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) => return Ok(None),
                        other => other?,
                    }
                    Ok(Some(path_of(&dialog.GetResult()?)?))
                }
            }
        })();
        match result {
            Ok(path) => path,
            Err(e) => {
                eprintln!("could not show the file question: {e}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_lists_its_own_type_first_and_all_files_second() {
        for kind in [Kind::Plan, Kind::Har, Kind::Results, Kind::Properties, Kind::Data] {
            let list = filters(kind);
            assert_eq!(list.len(), 2, "{kind:?}");
            assert_eq!(list[1], ALL_FILES, "{kind:?}");
            assert_ne!(list[0].patterns, "*.*", "{kind:?} lists its own files first");
        }
        assert_eq!(filters(Kind::Plan)[0], Filter { label: "JMeter file (*.jmx)", patterns: "*.jmx" });
        assert_eq!(filters(Kind::Results)[0].patterns, "*.jtl;*.csv");
    }

    #[test]
    fn patterns_are_plain_and_labels_say_what_they_show() {
        for kind in [Kind::Plan, Kind::Har, Kind::Results, Kind::Properties, Kind::Data] {
            for f in filters(kind) {
                assert!(!f.label.is_empty());
                assert!(f.patterns.split(';').all(|p| p.starts_with("*.") && p.len() > 2 && !p.contains(' ') && !p.contains('/') && !p.contains('\\')), "{f:?}");
            }
        }
    }

    #[test]
    fn a_name_without_an_extension_gets_the_kinds_extension() {
        assert_eq!(with_default_extension(Path::new("C:/plans/test"), Kind::Plan), PathBuf::from("C:/plans/test.jmx"));
        assert_eq!(with_default_extension(Path::new("props"), Kind::Properties), PathBuf::from("props.properties"));
    }

    #[test]
    fn a_name_that_has_an_extension_is_left_alone() {
        assert_eq!(with_default_extension(Path::new("a/test.xml"), Kind::Plan), PathBuf::from("a/test.xml"));
        assert_eq!(with_default_extension(Path::new("a/test.JMX"), Kind::Plan), PathBuf::from("a/test.JMX"));
        assert_eq!(with_default_extension(Path::new("a/noext"), Kind::Har), PathBuf::from("a/noext"), "kinds with no default add nothing");
        assert_eq!(with_default_extension(Path::new("a/.hidden"), Kind::Plan), PathBuf::from("a/.hidden.jmx"), "a leading dot is part of the name, not an extension");
    }

    #[test]
    fn only_plans_and_properties_have_a_default_extension() {
        assert_eq!(default_extension(Kind::Plan), Some("jmx"));
        assert_eq!(default_extension(Kind::Properties), Some("properties"));
        for kind in [Kind::Har, Kind::Results, Kind::Data] {
            assert_eq!(default_extension(kind), None);
        }
    }

    /// The system itself accepts the type list for every kind, for Open and for Save, and selects the first entry.
    #[cfg(windows)]
    #[test]
    fn the_system_accepts_the_type_list_for_every_kind_of_file() {
        for kind in [Kind::Plan, Kind::Har, Kind::Results, Kind::Properties, Kind::Data] {
            for saving in [false, true] {
                let index = std::thread::spawn(move || check_type_list(kind, saving)).join().expect("no panic");
                assert_eq!(index, Ok(1), "{kind:?} saving={saving}");
            }
        }
    }
}
