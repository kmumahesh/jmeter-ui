//! Small building blocks shared by the views.

use crate::theme::{self, with_alpha};
use gpui::{AnyElement, App, Hsla, IntoElement, ParentElement, SharedString, Styled, div, px, svg};

pub fn icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg().path(path).size(px(size)).flex_none().text_color(color)
}

/// A small rounded label such as a category name.
pub fn chip(text: impl Into<SharedString>, color: Hsla) -> AnyElement {
    div()
        .px(px(7.))
        .py(px(1.))
        .rounded(px(4.))
        .bg(with_alpha(color, 0.16))
        .text_color(color)
        .text_size(px(11.))
        .child(text.into())
        .into_any_element()
}

/// A heading above a block of content, in the small uppercase style used for panels.
pub fn section_title(text: &str, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    div()
        .px(px(12.))
        .pt(px(10.))
        .pb(px(6.))
        .text_size(px(11.))
        .font_family(crate::assets::MONO_FONT)
        .text_color(p.muted)
        .child(text.to_uppercase())
        .into_any_element()
}

/// A label on the left and a value on the right.
pub fn key_value(key: &str, value: impl IntoElement, cx: &App) -> AnyElement {
    let p = theme::palette(cx);
    div()
        .flex()
        .gap(px(10.))
        .px(px(12.))
        .py(px(3.))
        .text_size(px(12.5))
        .child(div().w(px(84.)).flex_none().text_color(p.muted).child(key.to_string()))
        .child(div().flex_1().min_w_0().child(value))
        .into_any_element()
}

/// Split `text` into runs, marking the characters at `positions` (character indexes).
pub fn highlight_runs(text: &str, positions: &[u32]) -> Vec<(String, bool)> {
    let mut runs: Vec<(String, bool)> = Vec::new();
    for (i, ch) in text.chars().enumerate() {
        let hit = positions.binary_search(&(i as u32)).is_ok();
        match runs.last_mut() {
            Some((s, h)) if *h == hit => s.push(ch),
            _ => runs.push((ch.to_string(), hit)),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_groups_adjacent_characters() {
        assert_eq!(highlight_runs("Loop Controller", &[0, 1, 5]), vec![("Lo".to_string(), true), ("op ".to_string(), false), ("C".to_string(), true), ("ontroller".to_string(), false)]);
    }

    #[test]
    fn highlight_without_positions_is_one_run() {
        assert_eq!(highlight_runs("abc", &[]), vec![("abc".to_string(), false)]);
        assert!(highlight_runs("", &[1]).is_empty());
    }

    #[test]
    fn highlight_counts_characters_not_bytes() {
        let runs = highlight_runs("caf\u{e9} x", &[3]);
        assert_eq!(runs, vec![("caf".to_string(), false), ("\u{e9}".to_string(), true), (" x".to_string(), false)]);
    }
}
