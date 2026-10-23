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
    let out = quiet(Command::new(path).arg("-version")).output().with_context(|| format!("cannot run {}", path.display()))?;
    // `java -version` prints to stderr.
    let text = String::from_utf8_lossy(&out.stderr).to_string();
    let first = text.lines().next().unwrap_or("").trim().to_string();
    let major = parse_java_major(&first).with_context(|| format!("cannot read the Java version from: {first:?}"))?;
    Ok(JavaInfo { path: path.to_path_buf(), version_line: first, major })
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
}
