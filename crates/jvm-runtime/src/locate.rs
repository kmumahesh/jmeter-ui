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
