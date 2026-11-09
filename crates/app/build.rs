//! Gives `jmeter-ui.exe` its icon, made from the one icon file `assets/app-icon/icon.svg`. The same icon is the one the
//! window and the taskbar show (the window toolkit asks the program for resource 1). Other systems need nothing here.

#[path = "build_support/icon.rs"]
mod icon;

use std::path::PathBuf;

fn main() {
    let svg_path = PathBuf::from("../../assets/app-icon/icon.svg");
    println!("cargo:rerun-if-changed={}", svg_path.display());
    println!("cargo:rerun-if-changed=build_support/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let svg = std::fs::read(&svg_path).unwrap_or_else(|e| panic!("cannot read the app icon {}: {e}", svg_path.display()));
    let ico = icon::build_ico(&svg).unwrap_or_else(|e| panic!("{e}"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let ico_path = out.join("app.ico");
    std::fs::write(&ico_path, ico).expect("write the icon file");
    // Resource 1 is the one the window toolkit loads for the window and the taskbar; the lowest number is also the icon File Explorer shows.
    let rc_path = out.join("app.rc");
    std::fs::write(&rc_path, format!("1 ICON \"{}\"\n", ico_path.display().to_string().replace('\\', "/"))).expect("write the resource file");
    embed_resource::compile(&rc_path, embed_resource::NONE).manifest_required().expect("put the icon in the program");
}
