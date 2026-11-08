//! The release program must be a Windows window program, so starting it does not open a terminal window. The check reads
//! the "subsystem" field of the program file's header (2 is a window program, 3 is a console program).

/// The subsystem value of a Windows program file, or None when the bytes are not a readable program header.
fn pe_subsystem(bytes: &[u8]) -> Option<u16> {
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
        return None;
    }
    let pe = u32::from_le_bytes(bytes[0x3C..0x40].try_into().ok()?) as usize;
    let field = pe.checked_add(24 + 68)?;
    if bytes.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    let raw = bytes.get(field..field + 2)?;
    Some(u16::from_le_bytes([raw[0], raw[1]]))
}

/// A minimal program header with the given subsystem, enough for the reader.
fn fake_program(subsystem: u16) -> Vec<u8> {
    let mut b = vec![0u8; 0x200];
    b[0] = b'M';
    b[1] = b'Z';
    b[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    b[0x80..0x84].copy_from_slice(b"PE\0\0");
    let at = 0x80 + 24 + 68;
    b[at..at + 2].copy_from_slice(&subsystem.to_le_bytes());
    b
}

#[test]
fn reads_window_and_console_programs() {
    assert_eq!(pe_subsystem(&fake_program(2)), Some(2));
    assert_eq!(pe_subsystem(&fake_program(3)), Some(3));
}

#[test]
fn damaged_headers_give_nothing_and_never_panic() {
    assert_eq!(pe_subsystem(&[]), None);
    assert_eq!(pe_subsystem(b"MZ"), None);
    assert_eq!(pe_subsystem(&vec![0u8; 0x200]), None);
    let mut far = fake_program(2);
    far[0x3C..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(pe_subsystem(&far), None);
    let mut no_pe = fake_program(2);
    no_pe[0x80] = b'X';
    assert_eq!(pe_subsystem(&no_pe), None);
    let cut = fake_program(2);
    assert_eq!(pe_subsystem(&cut[..0x80 + 24 + 69]), None);
    for len in 0..cut.len().min(0x120) {
        let _ = pe_subsystem(&cut[..len]);
    }
}

/// A release program, built by the release steps (set JMETER_UI_RELEASE_EXE to its path), is a window program.
#[test]
fn the_release_program_is_a_window_program() {
    let Ok(path) = std::env::var("JMETER_UI_RELEASE_EXE") else { return };
    let bytes = std::fs::read(&path).expect("release program");
    assert_eq!(pe_subsystem(&bytes), Some(2), "{path} would open a terminal window");
}

/// The check script that the release workflow runs passes a window program and stops on a console program.
#[cfg(windows)]
mod script {
    use super::*;
    use std::process::Command;

    fn run_check(exe: &std::path::Path) -> (bool, String) {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/check-subsystem.ps1");
        let out = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(script)
            .arg("-Exe")
            .arg(exe)
            .output()
            .expect("powershell");
        (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
    }

    fn temp_program(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jmeter-ui-subsystem-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn script_accepts_a_window_program() {
        let (ok, text) = run_check(&temp_program("window.exe", &fake_program(2)));
        assert!(ok, "{text}");
    }

    #[test]
    fn script_rejects_a_console_program() {
        let (ok, text) = run_check(&temp_program("console.exe", &fake_program(3)));
        assert!(!ok && text.contains("terminal window"), "{text}");
    }

    #[test]
    fn script_rejects_a_file_that_is_not_a_program() {
        let (ok, _) = run_check(&temp_program("text.exe", b"not a program, only text, long enough to pass the size test......"));
        assert!(!ok);
    }

    /// The test build keeps its console (for development output), so the script must call it a console program.
    #[test]
    fn the_debug_build_is_a_console_program() {
        let exe = std::path::Path::new(env!("CARGO_BIN_EXE_jmeter-ui"));
        let bytes = std::fs::read(exe).unwrap();
        assert_eq!(pe_subsystem(&bytes), Some(3));
        let (ok, _) = run_check(exe);
        assert!(!ok);
    }
}
