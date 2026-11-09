//! The app icon: one file, drawn at every size, put in the program, and nowhere else.

#[path = "../build_support/icon.rs"]
mod icon;

use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn master() -> PathBuf {
    repo().join("assets/app-icon/icon.svg")
}

/// Every file under the repository that is part of it (not build output, not git's own files).
fn repo_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if !matches!(name.as_str(), "target" | ".git" | ".claude" | "node_modules") {
                    walk(&p, out);
                }
            } else {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo(), &mut out);
    out
}

fn pixel(pm: &resvg::tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = pm.pixel(x, y).unwrap();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

#[test]
fn the_icon_is_one_valid_svg() {
    let svg = std::fs::read(master()).expect("assets/app-icon/icon.svg");
    let text = String::from_utf8(svg.clone()).unwrap();
    assert!(text.contains("<svg"), "not an SVG");
    assert!(!text.contains("<image") && !text.contains("href=\"http"), "the icon must be drawn, not fetched or copied from a picture");
    icon::render_png(&svg, 64).expect("it draws");
}

#[test]
fn it_draws_at_every_size_with_a_see_through_corner_and_a_filled_middle() {
    let svg = std::fs::read(master()).unwrap();
    for size in icon::SIZES {
        let png = icon::render_png(&svg, size).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "{size}: not a PNG");
        let pm = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        assert_eq!((pm.width(), pm.height()), (size, size));
        assert_eq!(pixel(&pm, 0, 0)[3], 0, "{size}: the corner is see-through (a rounded square)");
        let mid = pixel(&pm, size / 2, size / 2);
        assert!(mid[3] > 200, "{size}: the middle is filled: {mid:?}");
        let white = (0..size).flat_map(|y| (0..size).map(move |x| (x, y))).filter(|&(x, y)| {
            let c = pixel(&pm, x, y);
            c[3] > 200 && c[0] > 235 && c[1] > 235 && c[2] > 235
        });
        assert!(white.count() as u32 >= size / 4, "{size}: the white line is visible");
    }
}

#[test]
fn the_windows_icon_file_holds_every_size() {
    let svg = std::fs::read(master()).unwrap();
    let ico = icon::build_ico(&svg).unwrap();
    assert_eq!(&ico[..6], &[0, 0, 1, 0, icon::SIZES.len() as u8, 0], "header: icon, with one picture per size");
    for (i, size) in icon::SIZES.iter().enumerate() {
        let e = &ico[6 + 16 * i..6 + 16 * (i + 1)];
        let side = if e[0] == 0 { 256 } else { e[0] as u32 };
        assert_eq!((side, e[1] as u32 | if e[1] == 0 { 256 } else { 0 }), (*size, *size));
        let len = u32::from_le_bytes(e[8..12].try_into().unwrap()) as usize;
        let at = u32::from_le_bytes(e[12..16].try_into().unwrap()) as usize;
        assert!(at + len <= ico.len(), "{size}: inside the file");
        assert_eq!(&ico[at..at + 8], b"\x89PNG\r\n\x1a\n", "{size}: a PNG");
        assert_eq!(&ico[at..at + len], icon::render_png(&svg, *size).unwrap().as_slice());
    }
}

#[test]
fn the_icon_is_kept_in_one_place_and_nothing_else_is_a_copy_of_it() {
    let master_path = master();
    let svg = std::fs::read(&master_path).unwrap();
    let rendered: Vec<Vec<u8>> = icon::SIZES.iter().map(|&s| icon::render_png(&svg, s).unwrap()).collect();
    for f in repo_files() {
        if f == master_path {
            continue;
        }
        let ext = f.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        assert_ne!(ext, "ico", "{} is a second icon file: the .ico is made by the build from assets/app-icon/icon.svg", f.display());
        if matches!(ext.as_str(), "svg" | "png") {
            let bytes = std::fs::read(&f).unwrap();
            assert_ne!(bytes, svg, "{} is a copy of the app icon", f.display());
            assert!(!rendered.contains(&bytes), "{} is a picture of the app icon", f.display());
        }
    }
}

#[test]
fn the_title_bar_the_welcome_screen_and_the_build_all_use_that_one_file() {
    let src = std::fs::read_to_string(repo().join("crates/app/src/assets.rs")).unwrap();
    assert!(src.contains("pub const APP_ICON: &str = \"app-icon/icon.svg\";"), "one constant names the icon");
    let build = std::fs::read_to_string(repo().join("crates/app/build.rs")).unwrap();
    assert!(build.contains("assets/app-icon/icon.svg"), "the build makes the exe icon from the same file");
    for file in ["views.rs", "views_content.rs"] {
        let text = std::fs::read_to_string(repo().join("crates/app/src").join(file)).unwrap();
        assert!(!text.contains("app-icon/icon.svg"), "{file} names the file itself instead of using APP_ICON");
    }
    let uses: usize = ["views.rs", "views_content.rs"].iter().map(|f| std::fs::read_to_string(repo().join("crates/app/src").join(f)).unwrap().matches("APP_ICON").count()).sum();
    assert!(uses >= 3, "the title bar, the welcome screen and the About box draw the icon: {uses}");
}

#[test]
fn the_icon_is_free_to_use_and_says_where_it_came_from() {
    let readme = std::fs::read_to_string(repo().join("assets/app-icon/README.md")).unwrap();
    assert!(readme.contains("GPL-3.0-or-later") && readme.contains("drawn for this project"), "{readme}");
}

#[cfg(windows)]
#[test]
fn the_built_program_carries_the_icon_made_from_the_file() {
    use windows::Win32::UI::Shell::ExtractIconExW;
    use windows::core::HSTRING;
    let exe = env!("CARGO_BIN_EXE_jmeter-ui");
    let count = unsafe { ExtractIconExW(&HSTRING::from(exe), -1, None, None, 0) };
    assert!(count >= 1, "jmeter-ui.exe has no icon");
    // The 256 pixel picture in the program is the one drawn from the file today, byte for byte.
    let svg = std::fs::read(master()).unwrap();
    let want = icon::render_png(&svg, 256).unwrap();
    let bytes = std::fs::read(exe).unwrap();
    let found = bytes.windows(want.len()).step_by(1).any(|w| w[..16] == want[..16] && w == want.as_slice());
    assert!(found, "the icon in jmeter-ui.exe is not the one in assets/app-icon/icon.svg (build the program again)");
}
