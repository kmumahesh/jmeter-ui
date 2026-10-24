//! A small line chart drawn with the window's own painting, for the live numbers of a run.

use gpui::{Hsla, IntoElement, PathBuilder, Pixels, Point, canvas, div, fill, point, prelude::*, px, size};

/// The scale of a chart: the largest value, never below 1 so an empty chart does not divide by zero.
pub fn scale_max(values: &[f32]) -> f32 {
    values.iter().copied().filter(|v| v.is_finite()).fold(1.0_f32, f32::max)
}

/// Where each value sits in a box `w` wide and `h` high (y down), spread evenly from left to right.
pub fn plot_points(values: &[f32], max: f32, w: f32, h: f32) -> Vec<(f32, f32)> {
    let n = values.len();
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = if n <= 1 { w } else { w * i as f32 / (n - 1) as f32 };
            let v = if v.is_finite() { v.clamp(0.0, max) } else { 0.0 };
            (x, h - h * v / max)
        })
        .collect()
}

/// A chart of `values` in `color`, with two guide lines in `grid`.
pub fn line_chart(values: Vec<f32>, color: Hsla, grid: Hsla) -> impl IntoElement {
    let max = scale_max(&values);
    div().w_full().h(px(46.)).child(
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                if w < 2.0 || h < 2.0 {
                    return;
                }
                for frac in [0.0_f32, 0.5, 1.0] {
                    let y = (h - 1.0) * frac;
                    window.paint_quad(fill(gpui::Bounds { origin: point(bounds.origin.x, bounds.origin.y + px(y)), size: size(bounds.size.width, px(1.)) }, grid));
                }
                let pts = plot_points(&values, max, w, h - 2.0);
                let at = |&(x, y): &(f32, f32)| -> Point<Pixels> { point(bounds.origin.x + px(x), bounds.origin.y + px(y + 1.0)) };
                match pts.as_slice() {
                    [] => {}
                    [only] => {
                        let p = at(only);
                        window.paint_quad(fill(gpui::Bounds { origin: point(p.x - px(2.), p.y - px(2.)), size: size(px(4.), px(4.)) }, color));
                    }
                    [first, rest @ ..] => {
                        let mut path = PathBuilder::stroke(px(1.5));
                        path.move_to(at(first));
                        for p in rest {
                            path.line_to(at(p));
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    }
                }
            },
        )
        .size_full(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_is_the_largest_value_but_at_least_one() {
        assert_eq!(scale_max(&[]), 1.0);
        assert_eq!(scale_max(&[0.0, 0.2]), 1.0);
        assert_eq!(scale_max(&[3.0, 40.0, 7.0]), 40.0);
        assert_eq!(scale_max(&[f32::NAN, f32::INFINITY, 5.0]), 5.0, "values that are not numbers are ignored");
    }

    #[test]
    fn points_are_spread_across_the_width_with_zero_at_the_bottom() {
        let p = plot_points(&[0.0, 10.0, 5.0], 10.0, 100.0, 40.0);
        assert_eq!(p, vec![(0.0, 40.0), (50.0, 0.0), (100.0, 20.0)]);
        assert_eq!(plot_points(&[7.0], 10.0, 100.0, 40.0), vec![(100.0, 12.0)], "one value sits at the right edge");
        assert!(plot_points(&[], 1.0, 10.0, 10.0).is_empty());
    }

    #[test]
    fn values_outside_the_scale_stay_inside_the_chart() {
        let p = plot_points(&[-5.0, 99.0, f32::NAN], 10.0, 10.0, 10.0);
        assert!(p.iter().all(|&(x, y)| (0.0..=10.0).contains(&x) && (0.0..=10.0).contains(&y)), "{p:?}");
    }
}
