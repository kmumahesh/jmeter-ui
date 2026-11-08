//! The Run and Stop buttons in the title bar: what they say and how they look in each state.
//!
//! Run is a green play triangle with its name, Stop a red square with its name. Each is clearly "off" when it cannot be used,
//! and while a test is going Run says "Running" so it is plain that something is on.

use crate::theme::{Palette, with_alpha};
use gpui::Hsla;

/// The states a title bar button can be in.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Look {
    /// Run, ready to start a run.
    RunReady,
    /// Run, not available (no plan, or a run is going).
    RunOff,
    /// Run while a test is going: it shows "Running".
    Running,
    /// Stop, with a run to stop.
    StopReady,
    /// Stop, with nothing to stop.
    StopOff,
}

/// The colours of a button in one state.
#[derive(Copy, Clone, Debug)]
pub struct Colors {
    pub fill: Hsla,
    pub hover: Hsla,
    pub text: Hsla,
    pub icon: Hsla,
}

/// The name on the Run button.
pub fn run_label(running: bool) -> &'static str {
    if running { "Running" } else { "Run" }
}

/// Which look the Run button has.
pub fn run_look(can_run: bool, running: bool) -> Look {
    if running {
        Look::Running
    } else if can_run {
        Look::RunReady
    } else {
        Look::RunOff
    }
}

/// The look of the Stop button.
pub fn stop_look(can_stop: bool) -> Look {
    if can_stop { Look::StopReady } else { Look::StopOff }
}

/// The green and the red of an icon: the palette's own on the dark theme; darker on the light theme, where the palette's
/// lighter status colours would be too faint on the tinted button.
fn icon_color(p: &Palette, c: Hsla) -> Hsla {
    if p.dark { c } else { Hsla { l: (c.l - 0.16).clamp(0.0, 1.0), ..c } }
}

/// The colours for `look`. The icon is green or red; the name stays in the normal text colour so it reads well on the
/// tinted fill. An unavailable button is flat and grey, with no hover change.
pub fn colors(p: &Palette, look: Look) -> Colors {
    let off = Colors { fill: with_alpha(p.muted, 0.0), hover: with_alpha(p.muted, 0.0), text: p.muted, icon: p.muted };
    match look {
        Look::RunReady => Colors { fill: with_alpha(p.ok, 0.20), hover: with_alpha(p.ok, 0.32), text: p.fg, icon: icon_color(p, p.ok) },
        Look::Running => Colors { fill: with_alpha(p.ok, 0.20), hover: with_alpha(p.ok, 0.20), text: p.fg, icon: icon_color(p, p.ok) },
        Look::StopReady => Colors { fill: with_alpha(p.err, 0.20), hover: with_alpha(p.err, 0.32), text: p.fg, icon: icon_color(p, p.err) },
        Look::RunOff | Look::StopOff => off,
    }
}

/// The tooltip of a button in a state: what it does, or why it cannot.
pub fn tooltip(look: Look) -> &'static str {
    match look {
        Look::RunReady => "Run the saved plan (F5)",
        Look::RunOff => "Open a plan to run it (F5)",
        Look::Running => "A run is going. Stop it with Stop (Shift+F5).",
        Look::StopReady => "Stop the run after the current iteration (Shift+F5)",
        Look::StopOff => "Nothing is running",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::contrast;

    /// `fill` as it shows on `surface` (the fills have some transparency).
    fn on_surface(fill: Hsla, surface: Hsla) -> Hsla {
        let (f, s) = (fill.to_rgb(), surface.to_rgb());
        let mix = |a: f32, b: f32| a * fill.a + b * (1.0 - fill.a);
        gpui::Rgba { r: mix(f.r, s.r), g: mix(f.g, s.g), b: mix(f.b, s.b), a: 1.0 }.into()
    }

    fn palettes() -> [Palette; 2] {
        [Palette::dark(), Palette::light()]
    }

    #[test]
    fn the_run_button_says_running_while_a_test_is_going() {
        assert_eq!(run_label(false), "Run");
        assert_eq!(run_label(true), "Running");
    }

    #[test]
    fn each_state_has_the_look_it_should() {
        assert_eq!(run_look(true, false), Look::RunReady);
        assert_eq!(run_look(false, false), Look::RunOff);
        assert_eq!(run_look(false, true), Look::Running, "a run in progress shows Running even though Run cannot be pressed");
        assert_eq!(run_look(true, true), Look::Running);
        assert_eq!(stop_look(true), Look::StopReady);
        assert_eq!(stop_look(false), Look::StopOff);
    }

    #[test]
    fn run_is_green_and_stop_is_red_and_they_are_not_confused() {
        for p in palettes() {
            assert_eq!(colors(&p, Look::RunReady).icon.h, p.ok.h, "the same green, only darker on the light theme");
            assert_eq!(colors(&p, Look::StopReady).icon.h, p.err.h, "the same red");
            assert_ne!(colors(&p, Look::RunReady).icon, colors(&p, Look::StopReady).icon);
        }
    }

    #[test]
    fn names_and_icons_are_readable_on_the_bar_in_every_state_and_theme() {
        for p in palettes() {
            let mode = if p.dark { "dark" } else { "light" };
            for look in [Look::RunReady, Look::Running, Look::StopReady] {
                let c = colors(&p, look);
                for (state, fill, text_needs) in [("normal", c.fill, 7.0), ("hover", c.hover, 4.5)] {
                    let shown = on_surface(fill, p.panel);
                    assert!(contrast(c.text, shown) >= text_needs, "{mode} {look:?} {state}: name {:.2}", contrast(c.text, shown));
                    assert!(contrast(c.icon, shown) >= 3.0, "{mode} {look:?} {state}: icon {:.2}", contrast(c.icon, shown));
                }
            }
            for look in [Look::RunOff, Look::StopOff] {
                let c = colors(&p, look);
                assert!(contrast(c.text, p.panel) >= 3.0, "{mode} {look:?}: an unavailable button is still readable, {:.2}", contrast(c.text, p.panel));
                assert!(contrast(c.text, p.fg) > 1.0 && c.text == c.icon);
            }
        }
    }

    #[test]
    fn an_unavailable_button_is_flat_and_does_not_change_under_the_pointer() {
        for p in palettes() {
            for look in [Look::RunOff, Look::StopOff] {
                let c = colors(&p, look);
                assert_eq!(c.fill, c.hover);
                assert_eq!(c.fill.a, 0.0, "no fill");
            }
            assert_eq!(colors(&p, Look::Running).fill, colors(&p, Look::Running).hover, "Running is not pressed again");
            assert!(colors(&p, Look::RunReady).hover.a > colors(&p, Look::RunReady).fill.a, "a ready button reacts to the pointer");
        }
    }

    #[test]
    fn every_state_has_a_tooltip_that_says_what_it_does_or_why_not() {
        let all = [Look::RunReady, Look::RunOff, Look::Running, Look::StopReady, Look::StopOff];
        for look in all {
            assert!(!tooltip(look).is_empty());
        }
        assert!(tooltip(Look::RunReady).contains("F5"));
        assert!(tooltip(Look::StopReady).contains("Shift+F5"));
        let mut seen: Vec<&str> = all.iter().map(|l| tooltip(*l)).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), all.len(), "each state says something different");
    }
}
