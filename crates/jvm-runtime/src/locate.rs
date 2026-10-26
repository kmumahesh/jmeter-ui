//! Locating a Java runtime and a JMeter install, with the checks the Runtime screen shows.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct JavaInfo {
    pub path: PathBuf,
    /// First line of `java -version`, for example `openjdk version "21.0.8" 2025-07-15 LTS`.
    pub version_line: String,
    pub major: u32,
}

#[derive(Debug, Clone)]
pub struct JMeterInfo {
    pub home: PathBuf,
    pub jar: PathBuf,
    pub version: String,
    /// Names of the jars in `lib/ext`.
    pub ext_jars: Vec<String>,
}

pub fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Look for Java: an explicit path, then `JAVA_HOME`, then `PATH`.
pub fn find_java(explicit: Option<&Path>) -> Result<JavaInfo> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = explicit {
        candidates.push(p.to_path_buf());
    }
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        candidates.push(Path::new(&h).join("bin").join(exe));
    }
    candidates.push(PathBuf::from(exe));
    let mut last_err = None;
    for c in candidates {
        match probe_java(&c) {
            Ok(info) => return Ok(info),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap()).context("no working Java found (checked the setting, JAVA_HOME and PATH)")
}

fn probe_java(path: &Path) -> Result<JavaInfo> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "bat" | "cmd" | "com" | "ps1" | "vbs" | "js") {
        bail!("{} is a script, not a Java program", path.display());
    }
    let out = quiet(Command::new(path).arg("-version")).output().with_context(|| format!("cannot run {}", path.display()))?;
    // `java -version` prints to stderr.
    let text = String::from_utf8_lossy(&out.stderr).to_string();
    let first = text.lines().next().unwrap_or("").trim().to_string();
    let major = parse_java_major(&first).with_context(|| format!("cannot read the Java version from: {first:?}"))?;
    Ok(JavaInfo { path: resolve_on_path(path), version_line: first, major })
}

/// The full path of a program given by a bare name such as `java.exe`, found on `PATH`; any other path is
/// returned as it is.
pub fn resolve_on_path(program: &Path) -> PathBuf {
    if program.components().count() != 1 {
        return program.to_path_buf();
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|d| d.join(program))
        .find(|c| c.is_file())
        .unwrap_or_else(|| program.to_path_buf())
}

pub fn parse_java_major(line: &str) -> Option<u32> {
    let start = line.find('"')? + 1;
    let rest = &line[start..];
    let end = rest.find('"')?;
    let v = &rest[..end];
    let mut parts = v.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

/// A folder that holds a JMeter install: it has `bin/ApacheJMeter.jar`.
fn is_jmeter_home(dir: &Path) -> bool {
    dir.join("bin").join("ApacheJMeter.jar").is_file()
}

/// The version in a folder name such as `apache-jmeter-5.6.3`, as numbers to compare.
fn folder_version(name: &str) -> Option<Vec<u32>> {
    let v = name.strip_prefix("apache-jmeter-")?;
    let nums: Vec<u32> = v.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    (!nums.is_empty()).then_some(nums)
}

/// Look for a JMeter install without being told where: the folder named by `JMETER_HOME`, then folders named
/// `apache-jmeter-X.Y.Z` directly inside any of `roots`, the newest version first. Nothing is run and nothing outside
/// those folders is read.
pub fn find_jmeter_home_in(env_home: Option<&Path>, roots: &[PathBuf]) -> Option<PathBuf> {
    if let Some(h) = env_home.filter(|h| is_jmeter_home(h)) {
        return Some(h.to_path_buf());
    }
    let mut found: Vec<(Vec<u32>, PathBuf)> = Vec::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(root) else { continue };
        for e in rd.flatten().take(5000) {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(v) = folder_version(&name)
                && is_jmeter_home(&e.path())
            {
                found.push((v, e.path()));
            }
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().next().map(|(_, p)| p)
}

/// The usual places a JMeter download is unpacked on this computer.
pub fn usual_jmeter_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    if let Some(h) = &home {
        roots.extend([h.join("Downloads"), h.clone(), h.join("Desktop"), h.join("tools"), h.join("Documents")]);
    }
    if cfg!(windows) {
        roots.extend(["C:\\", "C:\\tools", "C:\\Program Files", "C:\\Program Files (x86)", "D:\\", "D:\\tools"].map(PathBuf::from));
    } else {
        roots.extend(["/opt", "/usr/local", "/usr/share", "/Applications"].map(PathBuf::from));
    }
    roots
}

/// The JMeter install to use when none was chosen.
pub fn find_jmeter_home() -> Option<PathBuf> {
    let env = std::env::var_os("JMETER_HOME").map(PathBuf::from);
    find_jmeter_home_in(env.as_deref(), &usual_jmeter_roots())
}

/// Check a JMeter install and read its version by running `ApacheJMeter.jar --version`.
pub fn inspect_jmeter(home: &Path, java: &JavaInfo) -> Result<JMeterInfo> {
    let jar = home.join("bin").join("ApacheJMeter.jar");
    if !jar.is_file() {
        bail!("{} not found", jar.display());
    }
    let out = quiet(Command::new(&java.path).arg("-jar").arg(&jar).arg("--version")).output().context("cannot run JMeter")?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let version = text
        .split_whitespace()
        .find(|w| w.contains('.') && w.starts_with(|c: char| c.is_ascii_digit()) && w.chars().all(|c| c.is_ascii_digit() || c == '.'))
        .unwrap_or("unknown")
        .to_string();
    let mut ext_jars: Vec<String> = std::fs::read_dir(home.join("lib").join("ext"))
        .map(|rd| rd.flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.ends_with(".jar")).collect())
        .unwrap_or_default();
    ext_jars.sort();
    Ok(JMeterInfo { home: home.to_path_buf(), jar, version, ext_jars })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_home(root: &Path, name: &str) -> PathBuf {
        let h = root.join(name);
        std::fs::create_dir_all(h.join("bin")).unwrap();
        std::fs::write(h.join("bin").join("ApacheJMeter.jar"), b"jar").unwrap();
        h
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jmeter-ui-locate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_newest_install_in_the_usual_places_is_found() {
        let a = temp("roots-a");
        let b = temp("roots-b");
        fake_home(&a, "apache-jmeter-5.4.1");
        let newest = fake_home(&b, "apache-jmeter-5.6.3");
        fake_home(&b, "apache-jmeter-5.10.0-RC1x"); // not a version
        let ten = fake_home(&a, "apache-jmeter-5.10.0");
        // 5.10.0 is newer than 5.6.3: versions are compared as numbers, not as text.
        assert_eq!(find_jmeter_home_in(None, &[a.clone(), b.clone()]), Some(ten));
        assert_eq!(find_jmeter_home_in(None, &[b]), Some(newest));
        assert_eq!(find_jmeter_home_in(None, &[temp("roots-empty")]), None);
        assert_eq!(find_jmeter_home_in(None, &[PathBuf::from("no/such/folder")]), None);
    }

    #[test]
    fn a_folder_that_only_looks_like_jmeter_is_not_taken() {
        let r = temp("fake");
        std::fs::create_dir_all(r.join("apache-jmeter-5.6.3").join("bin")).unwrap(); // no jar
        std::fs::create_dir_all(r.join("jmeter")).unwrap();
        std::fs::write(r.join("apache-jmeter-9.9.9"), b"a file, not a folder").unwrap();
        assert_eq!(find_jmeter_home_in(None, &[r]), None);
    }

    #[test]
    fn jmeter_home_in_the_environment_comes_first_when_it_is_an_install() {
        let r = temp("env");
        let chosen = fake_home(&r, "my-jmeter");
        let other = temp("env-other");
        fake_home(&other, "apache-jmeter-5.6.3");
        assert_eq!(find_jmeter_home_in(Some(&chosen), std::slice::from_ref(&other)), Some(chosen));
        // An environment value that is not an install is ignored.
        let nothing = temp("env-bad");
        assert_eq!(find_jmeter_home_in(Some(&nothing), std::slice::from_ref(&other)).map(|p| p.ends_with("apache-jmeter-5.6.3")), Some(true));
    }

    #[test]
    fn folder_names_give_versions_that_compare_as_numbers() {
        assert_eq!(folder_version("apache-jmeter-5.6.3"), Some(vec![5, 6, 3]));
        assert!(folder_version("apache-jmeter-5.10.0") > folder_version("apache-jmeter-5.6.3"));
        for bad in ["apache-jmeter-", "apache-jmeter-x", "apache-jmeter-5.6.3-RC1", "jmeter-5.6.3", "apache-jmeter-5..3", ""] {
            assert_eq!(folder_version(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn java_versions() {
        assert_eq!(parse_java_major(r#"openjdk version "21.0.8" 2025-07-15 LTS"#), Some(21));
        assert_eq!(parse_java_major(r#"java version "1.8.0_391""#), Some(8));
        assert_eq!(parse_java_major(r#"openjdk version "17" 2021-09-14"#), Some(17));
        assert_eq!(parse_java_major("garbage"), None);
    }

    #[test]
    fn a_script_is_never_used_as_java() {
        for name in ["java.bat", "java.CMD", "x/java.ps1", "java.vbs"] {
            let err = probe_java(Path::new(name)).unwrap_err().to_string();
            assert!(err.contains("not a Java program"), "{name}: {err}");
        }
    }

    #[test]
    fn a_bare_program_name_is_found_on_the_path_and_other_paths_are_kept() {
        assert_eq!(resolve_on_path(Path::new("C:/x/java.exe")), Path::new("C:/x/java.exe"));
        assert_eq!(resolve_on_path(Path::new("no-such-program-anywhere.exe")), Path::new("no-such-program-anywhere.exe"));
        let found = resolve_on_path(Path::new(if cfg!(windows) { "cmd.exe" } else { "sh" }));
        assert!(found.is_absolute() && found.is_file(), "{found:?}");
    }
}
