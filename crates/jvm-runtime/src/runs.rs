//! Where the files of each run are kept, how old runs are removed, and how the shared folder is cleared.
//!
//! Every run gets its own folder, `<base>/runs/<plan name>-<date>-<time>/`, which holds `run.json` (what the
//! run was and how it ended) next to the files JMeter writes. `base` is the folder of the `.jmx` file or a
//! folder the user chose. Only folders that hold this app's own `run.json` are ever listed or removed.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

/// The name of the folder that holds the run folders.
pub const RUNS_DIR: &str = "runs";
/// The file in each run folder that describes the run.
pub const META_FILE: &str = "run.json";
/// Marks `run.json` as ours.
const APP_MARK: &str = "jmeter-ui";
/// A `run.json` larger than this is not read.
const META_MAX_BYTES: u64 = 64 * 1024;

/// What the run was for.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    /// The whole plan.
    Run,
    /// One thread, one iteration, no timers (JMeter's own validation rules).
    Validate,
    /// One request, with what applies to it.
    SendOnce,
}

impl RunKind {
    pub fn label(self) -> &'static str {
        match self {
            RunKind::Run => "Run",
            RunKind::Validate => "Validate",
            RunKind::SendOnce => "Send once",
        }
    }
}

/// How a run ended.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Not ended yet (or the app ended before it could write the end).
    Running,
    Finished,
    /// Stopped by the user.
    Stopped,
    Failed,
}

impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Outcome::Running => "Running",
            Outcome::Finished => "Finished",
            Outcome::Stopped => "Stopped",
            Outcome::Failed => "Failed",
        }
    }
}

/// `run.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunMeta {
    pub app: String,
    /// The plan file that was run.
    pub plan_path: String,
    pub plan_name: String,
    pub kind: RunKind,
    /// Milliseconds since 1970 (UTC) and the same time as local text for people reading the file.
    pub started_ms: u64,
    pub started: String,
    pub ended_ms: Option<u64>,
    pub outcome: Outcome,
    pub exit_code: Option<i32>,
    pub samples: u64,
    pub errors: u64,
    pub avg_ms: u64,
    pub p95_ms: u64,
    /// The `-J` overrides and other things worth knowing about the run.
    pub notes: Vec<String>,
}

impl RunMeta {
    pub fn new(plan: &Path, kind: RunKind, now: DateTime<Local>) -> RunMeta {
        RunMeta {
            app: APP_MARK.into(),
            plan_path: plan.to_string_lossy().into_owned(),
            plan_name: plan_name(plan),
            kind,
            started_ms: now.timestamp_millis().max(0) as u64,
            started: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            ended_ms: None,
            outcome: Outcome::Running,
            exit_code: None,
            samples: 0,
            errors: 0,
            avg_ms: 0,
            p95_ms: 0,
            notes: Vec::new(),
        }
    }

    pub fn duration_secs(&self) -> Option<f64> {
        self.ended_ms.map(|e| e.saturating_sub(self.started_ms) as f64 / 1000.0)
    }
}

/// A run folder and what `run.json` says about it.
#[derive(Clone, Debug)]
pub struct RunEntry {
    pub dir: PathBuf,
    pub meta: RunMeta,
}

/// Where the run folders go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// Next to the `.jmx` file.
    PlanFolder,
    /// A folder the user chose; every plan shares its `runs` folder.
    Folder(PathBuf),
}

impl Place {
    /// The setting values: a flag for "custom" and the chosen folder.
    pub fn from_setting(custom: bool, folder: Option<&str>) -> Place {
        match (custom, folder.filter(|f| !f.trim().is_empty())) {
            (true, Some(f)) => Place::Folder(PathBuf::from(f)),
            _ => Place::PlanFolder,
        }
    }
}

/// The local time type used by [`RunMeta::new`] and [`new_run`].
pub type LocalTime = DateTime<Local>;

/// The local time now.
pub fn local_now() -> LocalTime {
    Local::now()
}

/// Milliseconds since 1970 (UTC) now.
pub fn now_ms() -> u64 {
    Local::now().timestamp_millis().max(0) as u64
}

/// [`new_run`] for the current time.
pub fn new_run_now(root: &Path, plan: &Path, kind: RunKind) -> io::Result<(PathBuf, RunMeta)> {
    new_run(root, plan, kind, Local::now())
}

/// The name of a plan without its folder and `.jmx`.
pub fn plan_name(plan: &Path) -> String {
    plan.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "plan".into())
}

/// The `runs` folder for `plan`: inside the plan's own folder, or inside the chosen folder.
pub fn runs_root(plan: &Path, place: &Place) -> PathBuf {
    match place {
        Place::PlanFolder => plan.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(".")).join(RUNS_DIR),
        Place::Folder(f) => f.join(RUNS_DIR),
    }
}

/// Make sure run files can be kept for `plan` and return the `runs` folder. A custom folder that is gone or
/// cannot be written to is an error that says what is wrong, so the run can be stopped before it starts.
pub fn prepare_root(plan: &Path, place: &Place) -> Result<PathBuf, String> {
    if let Place::Folder(f) = place {
        match std::fs::metadata(f) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(format!("{} is not a folder", f.display())),
            Err(_) => return Err(format!("The folder {} does not exist", f.display())),
        }
    }
    let root = runs_root(plan, place);
    std::fs::create_dir_all(&root).map_err(|e| format!("Cannot create {}: {e}", root.display()))?;
    let probe = root.join(format!(".write-test-{}", std::process::id()));
    std::fs::write(&probe, b"").map_err(|e| format!("Cannot write to {}: {e}", root.display()))?;
    let _ = std::fs::remove_file(probe);
    Ok(root)
}

/// A folder name part from a plan name: letters, digits, `-`, `_` and `.` stay, everything else becomes `_`.
fn safe_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' }).collect();
    let s = s.trim_matches(|c| c == '.' || c == '_').chars().take(60).collect::<String>();
    if s.is_empty() { "plan".into() } else { s }
}

/// Make the folder for a new run inside `root` and write its first `run.json`. The folder is named after the
/// plan and the time; a second run in the same second gets a number added.
pub fn new_run(root: &Path, plan: &Path, kind: RunKind, now: DateTime<Local>) -> io::Result<(PathBuf, RunMeta)> {
    std::fs::create_dir_all(root)?;
    let base = format!("{}-{}", safe_name(&plan_name(plan)), now.format("%Y%m%d-%H%M%S"));
    for n in 1..1000 {
        let name = if n == 1 { base.clone() } else { format!("{base}-{n}") };
        let dir = root.join(name);
        match std::fs::create_dir(&dir) {
            Ok(()) => {
                let meta = RunMeta::new(plan, kind, now);
                write_meta(&dir, &meta)?;
                return Ok((dir, meta));
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::other("too many runs in the same second"))
}

pub fn write_meta(dir: &Path, meta: &RunMeta) -> io::Result<()> {
    let text = serde_json::to_string_pretty(meta).map_err(io::Error::other)?;
    let tmp = dir.join(format!("{META_FILE}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, dir.join(META_FILE))
}

/// `run.json` of a folder, if it is a run folder of this app.
pub fn read_meta(dir: &Path) -> Option<RunMeta> {
    let file = dir.join(META_FILE);
    let m = std::fs::symlink_metadata(&file).ok()?;
    if !m.is_file() || m.len() > META_MAX_BYTES {
        return None;
    }
    let meta: RunMeta = serde_json::from_slice(&std::fs::read(&file).ok()?).ok()?;
    (meta.app == APP_MARK).then_some(meta)
}

/// A path as one comparable text: the real path when the file exists (so a short Windows name such as
/// `RUNNER~1` and the long name match), otherwise the path as written; slashes and case are ignored.
fn path_key(p: &str) -> String {
    let real = std::fs::canonicalize(p).map(|c| c.to_string_lossy().into_owned()).unwrap_or_else(|_| p.to_string());
    real.replace('\\', "/").trim_start_matches("//?/").to_lowercase()
}

fn is_real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}

/// The runs in `root`, newest first. With `plan`, only the runs of that plan.
pub fn list_runs(root: &Path, plan: Option<&Path>) -> Vec<RunEntry> {
    let Ok(rd) = std::fs::read_dir(root) else { return Vec::new() };
    let want = plan.map(|p| path_key(&p.to_string_lossy()));
    let mut last: Option<(String, String)> = None; // plan path as stored, and its key: runs of one plan share it
    let mut out: Vec<RunEntry> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| is_real_dir(p))
        .filter_map(|dir| read_meta(&dir).map(|meta| RunEntry { dir, meta }))
        .filter(|r| match &want {
            None => true,
            Some(w) => {
                if last.as_ref().is_none_or(|(stored, _)| *stored != r.meta.plan_path) {
                    last = Some((r.meta.plan_path.clone(), path_key(&r.meta.plan_path)));
                }
                last.as_ref().is_some_and(|(_, k)| k == w)
            }
        })
        .collect();
    out.sort_by(|a, b| b.meta.started_ms.cmp(&a.meta.started_ms).then_with(|| b.dir.cmp(&a.dir)));
    out
}

/// A run whose `run.json` never got an end (the app was closed or crashed) is taken for still going for
/// this long; after that it is an old run like any other and can be removed.
pub const STALE_RUNNING_MS: u64 = 24 * 60 * 60 * 1000;

/// Remove the oldest runs of `plan` so that at most `keep` remain, with everything in their folders. A run
/// that has not ended is not removed unless it started more than a day ago. Returns the folders removed.
pub fn prune(root: &Path, plan: &Path, keep: usize) -> Vec<PathBuf> {
    prune_at(root, plan, keep, now_ms())
}

/// [`prune`] as of the time `now` (milliseconds since 1970).
pub fn prune_at(root: &Path, plan: &Path, keep: usize, now: u64) -> Vec<PathBuf> {
    let runs = list_runs(root, Some(plan));
    let mut removed = Vec::new();
    for r in runs.into_iter().skip(keep.max(1)) {
        if r.meta.outcome == Outcome::Running && now.saturating_sub(r.meta.started_ms) < STALE_RUNNING_MS {
            continue;
        }
        if std::fs::remove_dir_all(&r.dir).is_ok() {
            removed.push(r.dir);
        }
    }
    removed
}

/// Empty a shared `runs` folder: remove every run folder this app made in it. Anything else in the folder,
/// and anything behind a link, is left alone. Only a folder named `runs` is accepted.
pub fn clear_root(root: &Path) -> Result<usize, String> {
    if root.file_name().is_none_or(|n| n != RUNS_DIR) {
        return Err(format!("{} is not a folder named {RUNS_DIR}", root.display()));
    }
    if !is_real_dir(root) {
        return Ok(0);
    }
    let mut n = 0;
    for r in list_runs(root, None) {
        if std::fs::remove_dir_all(&r.dir).is_ok() {
            n += 1;
        }
    }
    Ok(n)
}

/// Total size in bytes of the files in a run folder (not following links).
pub fn folder_size(dir: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.filter_map(Result::ok)
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() && !t.is_symlink() => folder_size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map(|m| m.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jmeter-ui-runs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn at(s: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 24, 9, 30, s).unwrap()
    }

    #[test]
    fn runs_go_in_a_runs_folder_next_to_the_plan_or_inside_the_chosen_folder() {
        let plan = Path::new("C:/work/shop/checkout.jmx");
        assert_eq!(runs_root(plan, &Place::PlanFolder), Path::new("C:/work/shop").join("runs"));
        assert_eq!(runs_root(plan, &Place::Folder("D:/all-runs".into())), Path::new("D:/all-runs").join("runs"));
        assert_eq!(runs_root(Path::new("p.jmx"), &Place::PlanFolder), Path::new(".").join("runs"));
    }

    #[test]
    fn a_custom_place_needs_a_folder_otherwise_the_plan_folder_is_used() {
        assert_eq!(Place::from_setting(false, Some("D:/x")), Place::PlanFolder);
        assert_eq!(Place::from_setting(true, None), Place::PlanFolder);
        assert_eq!(Place::from_setting(true, Some("  ")), Place::PlanFolder);
        assert_eq!(Place::from_setting(true, Some("D:/x")), Place::Folder("D:/x".into()));
    }

    #[test]
    fn runs_of_a_plan_are_found_however_the_plan_path_is_spelled() {
        let d = dir("spelling");
        let plan = d.join("shop.jmx");
        std::fs::write(&plan, "x").unwrap();
        let root = d.join("runs");
        // Stored with one spelling of the path (a detour through a folder, as a short Windows name would be).
        std::fs::create_dir_all(d.join("sub")).unwrap();
        let detour = d.join("sub").join("..").join("shop.jmx");
        new_run(&root, &detour, RunKind::Run, at(1)).unwrap();
        assert_eq!(list_runs(&root, Some(&plan)).len(), 1, "found with the plain path");
        assert_eq!(list_runs(&root, Some(&detour)).len(), 1);
        assert_eq!(list_runs(&root, Some(&d.join("other.jmx"))).len(), 0, "another plan is not matched");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn preparing_makes_the_runs_folder_and_checks_it_can_be_written() {
        let d = dir("prepare");
        let plan = d.join("shop.jmx");
        let root = prepare_root(&plan, &Place::PlanFolder).unwrap();
        assert_eq!(root, d.join("runs"));
        assert!(root.is_dir());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0, "the write test leaves nothing behind");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_missing_or_wrong_custom_folder_is_reported() {
        let d = dir("badfolder");
        let plan = d.join("shop.jmx");
        let gone = prepare_root(&plan, &Place::Folder(d.join("gone"))).unwrap_err();
        assert!(gone.contains("does not exist"), "{gone}");
        std::fs::write(d.join("afile"), "x").unwrap();
        let file = prepare_root(&plan, &Place::Folder(d.join("afile"))).unwrap_err();
        assert!(file.contains("not a folder"), "{file}");
        assert!(!d.join("gone").exists(), "a missing chosen folder is not made up");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_new_run_gets_its_own_folder_named_after_the_plan_and_time() {
        let d = dir("newrun");
        let plan = d.join("My Shop (v2).jmx");
        let root = d.join("runs");
        let (a, meta) = new_run(&root, &plan, RunKind::Run, at(5)).unwrap();
        assert_eq!(a.file_name().unwrap().to_string_lossy(), "My_Shop__v2-20261024-093005");
        assert_eq!(meta.outcome, Outcome::Running);
        assert_eq!(read_meta(&a).unwrap(), meta);
        let (b, _) = new_run(&root, &plan, RunKind::Run, at(5)).unwrap();
        assert_ne!(a, b, "the same second gets a number");
        assert!(b.file_name().unwrap().to_string_lossy().ends_with("-2"));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn plan_names_that_try_to_leave_the_folder_stay_inside_it() {
        let d = dir("evilname");
        let root = d.join("runs");
        for name in ["..", "../../x", "a/b\\c", "C:evil", "CON", "  ", ""] {
            let (dir_, _) = new_run(&root, &PathBuf::from(format!("{name}.jmx")), RunKind::Run, at(1)).unwrap();
            assert_eq!(dir_.parent().unwrap(), root, "{name:?} escaped to {}", dir_.display());
        }
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn listing_shows_this_apps_run_folders_newest_first_and_ignores_the_rest() {
        let d = dir("list");
        let root = d.join("runs");
        let plan = d.join("shop.jmx");
        let other = d.join("cart.jmx");
        new_run(&root, &plan, RunKind::Run, at(1)).unwrap();
        new_run(&root, &plan, RunKind::Validate, at(3)).unwrap();
        new_run(&root, &other, RunKind::Run, at(2)).unwrap();
        std::fs::create_dir_all(root.join("my own folder")).unwrap();
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        std::fs::create_dir_all(root.join("not ours")).unwrap();
        std::fs::write(root.join("not ours").join(META_FILE), "{\"app\":\"someone else\"}").unwrap();

        let mine = list_runs(&root, Some(&plan));
        assert_eq!(mine.iter().map(|r| r.meta.kind).collect::<Vec<_>>(), [RunKind::Validate, RunKind::Run]);
        assert_eq!(list_runs(&root, None).len(), 3);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_damaged_or_huge_run_file_is_ignored() {
        let d = dir("damaged");
        let root = d.join("runs");
        let (a, _) = new_run(&root, &d.join("p.jmx"), RunKind::Run, at(1)).unwrap();
        std::fs::write(a.join(META_FILE), "{ not json").unwrap();
        assert!(read_meta(&a).is_none());
        std::fs::write(a.join(META_FILE), vec![b' '; 100_000]).unwrap();
        assert!(read_meta(&a).is_none());
        assert!(list_runs(&root, None).is_empty());
        let _ = std::fs::remove_dir_all(d);
    }

    fn finish(dir_: &Path, mut meta: RunMeta) {
        meta.outcome = Outcome::Finished;
        meta.ended_ms = Some(meta.started_ms + 5000);
        write_meta(dir_, &meta).unwrap();
    }

    #[test]
    fn only_the_newest_runs_of_a_plan_are_kept_with_all_their_files() {
        let d = dir("prune");
        let root = d.join("runs");
        let plan = d.join("shop.jmx");
        let other = d.join("cart.jmx");
        for s in 0..6 {
            let (dir_, meta) = new_run(&root, &plan, RunKind::Run, at(s)).unwrap();
            std::fs::write(dir_.join("results.jtl"), "x").unwrap();
            std::fs::write(dir_.join("jmeter.log"), "x").unwrap();
            finish(&dir_, meta);
        }
        let (c, m) = new_run(&root, &other, RunKind::Run, at(0)).unwrap();
        finish(&c, m);
        let removed = prune_at(&root, &plan, 4, 0);
        assert_eq!(removed.len(), 2);
        let left = list_runs(&root, Some(&plan));
        assert_eq!(left.len(), 4);
        assert_eq!(left[3].meta.started, "2026-10-24 09:30:02", "the oldest two went");
        for r in &left {
            assert!(r.dir.join("results.jtl").exists() && r.dir.join("jmeter.log").exists(), "kept runs keep their files");
        }
        assert_eq!(list_runs(&root, Some(&other)).len(), 1, "other plans are not touched");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_run_that_has_not_ended_is_never_pruned_and_keep_is_at_least_one() {
        let d = dir("prune-running");
        let root = d.join("runs");
        let plan = d.join("shop.jmx");
        new_run(&root, &plan, RunKind::Run, at(1)).unwrap(); // still "running"
        let (b, m) = new_run(&root, &plan, RunKind::Run, at(2)).unwrap();
        finish(&b, m);
        let started = list_runs(&root, Some(&plan)).last().unwrap().meta.started_ms;
        assert!(prune_at(&root, &plan, 0, started + 3_600_000).is_empty(), "keep 0 means keep 1, and the older run has not ended");
        assert_eq!(list_runs(&root, Some(&plan)).len(), 2);
        let gone = prune_at(&root, &plan, 0, started + STALE_RUNNING_MS + 1);
        assert_eq!(gone.len(), 1, "a run that never ended and is over a day old is removed like any other");
        assert_eq!(list_runs(&root, Some(&plan)).len(), 1);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn clearing_removes_only_this_apps_run_folders() {
        let d = dir("clear");
        let root = d.join("runs");
        for (name, s) in [("a", 1), ("b", 2)] {
            let (x, m) = new_run(&root, &d.join(format!("{name}.jmx")), RunKind::Run, at(s)).unwrap();
            std::fs::write(x.join("results.jtl"), "x").unwrap();
            finish(&x, m);
        }
        std::fs::create_dir_all(root.join("keep me")).unwrap();
        std::fs::write(root.join("keep me").join("file.txt"), "mine").unwrap();
        std::fs::write(root.join("note.txt"), "mine").unwrap();
        assert_eq!(clear_root(&root).unwrap(), 2);
        assert!(list_runs(&root, None).is_empty());
        assert!(root.join("keep me").join("file.txt").exists() && root.join("note.txt").exists(), "other files are left alone");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn clearing_refuses_a_folder_that_is_not_named_runs() {
        let d = dir("clear-refuse");
        let (x, m) = new_run(&d, &d.join("a.jmx"), RunKind::Run, at(1)).unwrap();
        finish(&x, m);
        assert!(clear_root(&d).is_err());
        assert!(clear_root(Path::new("C:/")).is_err());
        assert!(x.exists(), "nothing was removed");
        assert_eq!(clear_root(&d.join("runs")).unwrap(), 0, "a missing runs folder is already clear");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn clearing_never_follows_a_link_out_of_the_folder() {
        let d = dir("clear-link");
        let outside = d.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("precious.txt"), "x").unwrap();
        // Make the outside folder look like one of ours.
        write_meta(&outside, &RunMeta::new(&d.join("p.jmx"), RunKind::Run, at(1))).unwrap();
        let root = d.join("runs");
        std::fs::create_dir_all(&root).unwrap();
        let link = root.join("link");
        #[cfg(windows)]
        let made = std::process::Command::new("cmd").args(["/C", "mklink", "/J"]).arg(&link).arg(&outside).output().is_ok_and(|o| o.status.success());
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(&outside, &link).is_ok();
        if made {
            assert_eq!(clear_root(&root).unwrap(), 0, "the link is not a run folder");
            assert!(outside.join("precious.txt").exists(), "what the link points at is untouched");
            assert!(list_runs(&root, None).is_empty());
        }
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn folder_size_adds_up_the_files() {
        let d = dir("size");
        std::fs::create_dir_all(d.join("a")).unwrap();
        std::fs::write(d.join("a").join("x"), vec![0u8; 100]).unwrap();
        std::fs::write(d.join("y"), vec![0u8; 50]).unwrap();
        assert_eq!(folder_size(&d), 150);
        let _ = std::fs::remove_dir_all(d);
    }
}
