//! Turn the app icon (one SVG file) into the pictures a program needs. The build uses this to make the icon of
//! `jmeter-ui.exe`, and the tests use the same code to check that the icon draws at every size.

use resvg::{tiny_skia, usvg};

/// The sizes put in the Windows icon, smallest first. 16 is the title bar and file lists, 32 the taskbar, 48 and 256 File Explorer.
pub const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// The icon drawn `size` pixels wide and high, as a PNG with a see-through background.
pub fn render_png(svg: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).map_err(|e| format!("the icon is not a valid SVG: {e}"))?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or_else(|| format!("cannot make a {size} pixel picture"))?;
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| format!("cannot write the PNG: {e}"))
}

/// A Windows `.ico` file holding the icon at every size in [`SIZES`] (each as a PNG, which Windows Vista and later read).
pub fn build_ico(svg: &[u8]) -> Result<Vec<u8>, String> {
    let images: Vec<(u32, Vec<u8>)> = SIZES.iter().map(|&s| render_png(svg, s).map(|p| (s, p))).collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]); // reserved, type 1 = icon
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, png) in &images {
        let side = if *size >= 256 { 0 } else { *size as u8 }; // 0 means 256
        out.extend_from_slice(&[side, side, 0, 0]); // width, height, colours, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in &images {
        out.extend_from_slice(png);
    }
    Ok(out)
}
