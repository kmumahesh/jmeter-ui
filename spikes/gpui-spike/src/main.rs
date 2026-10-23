//! Spike 1: can GPUI carry this app on Windows?
//! Checks: custom title bar with window controls, text input (typing, clipboard), a 10,000-row
//! virtual list, a live chart drawn with paths, and bundled fonts. Run with `--bench` to scroll the
//! list automatically for a few seconds and write frame statistics to `bench.txt`.

mod text_input;

use gpui::{
    App, Bounds, Context, Entity, ScrollStrategy, TitlebarOptions, UniformListScrollHandle, Window, WindowBounds,
    WindowControlArea, WindowOptions, canvas, div, fill, point, prelude::*, px, rgb, size, uniform_list, PathBuilder,
};
use gpui_platform::application;
use std::borrow::Cow;
use std::ops::Range;
use std::time::{Duration, Instant};
use text_input::TextInput;

const BG: u32 = 0x16181d;
const PANEL: u32 = 0x1c1f26;
const PANEL2: u32 = 0x232730;
const LINE: u32 = 0x2e333d;
const FG: u32 = 0xd6dae2;
const MUTED: u32 = 0x8d95a5;
const ACCENT: u32 = 0x6f9fff;
const OK: u32 = 0x7fd28a;
const WARN: u32 = 0xe5c07b;
const ERR: u32 = 0xf07a84;

const ROWS: usize = 10_000;

struct Spike {
    palette: Entity<TextInput>,
    name: Entity<TextInput>,
    url: Entity<TextInput>,
    scroll: UniformListScrollHandle,
    selected: usize,
    bench: bool,
    started: Instant,
    last_frame: Instant,
    frames: u64,
    frame_ms_sum: f64,
    frame_ms_max: f64,
    scroll_pos: usize,
    last_point: Instant,
    avg: Vec<f32>,
    p95: Vec<f32>,
    seed: u32,
    fps_stamp: Instant,
    fps_frames: u64,
    fps: f32,
}

impl Spike {
    fn next_rand(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn push_point(&mut self) {
        let a = 0.35 + 0.12 * self.next_rand();
        let p = (a * 2.3 + 0.08 * self.next_rand()).min(0.98);
        self.avg.remove(0);
        self.avg.push(a);
        self.p95.remove(0);
        self.p95.push(p);
    }

    fn series_path(values: &[f32], bounds: Bounds<gpui::Pixels>, color: u32, window: &mut Window) {
        if values.len() < 2 {
            return;
        }
        let mut b = PathBuilder::stroke(px(1.6));
        for (i, v) in values.iter().enumerate() {
            let x = bounds.left() + bounds.size.width * (i as f32 / (values.len() - 1) as f32);
            let y = bounds.bottom() - bounds.size.height * *v;
            if i == 0 { b.move_to(point(x, y)) } else { b.line_to(point(x, y)) }
        }
        if let Ok(path) = b.build() {
            window.paint_path(path, rgb(color));
        }
    }

    fn row(selected_ix: usize, ix: usize, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let (method, color) = match ix % 3 {
            0 => ("GET", OK),
            1 => ("POST", WARN),
            _ => ("PUT", ACCENT),
        };
        let selected = ix == selected_ix;
        div()
            .id(ix)
            .h(px(24.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(px(12.5))
            .when(selected, |d| d.bg(rgb(0x2b3a57)))
            .hover(|d| d.bg(rgb(PANEL2)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = ix;
                cx.notify();
            }))
            .child(div().w(px(34.)).text_color(rgb(color)).font_family("Lilex").text_size(px(10.)).child(method))
            .child(format!("/api/items/{ix}"))
    }

    fn control(label: &'static str, area: WindowControlArea, hover: u32) -> impl IntoElement {
        div()
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(MUTED))
            .window_control_area(area)
            .hover(move |d| d.bg(rgb(hover)).text_color(rgb(FG)))
            .child(label)
    }
}

impl Render for Spike {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Frame statistics. Rendering continuously is the point of the benchmark.
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64() * 1000.0;
        self.last_frame = now;
        if self.frames > 0 {
            self.frame_ms_sum += dt;
            self.frame_ms_max = self.frame_ms_max.max(dt);
        }
        self.frames += 1;
        self.fps_frames += 1;
        if now.duration_since(self.fps_stamp) >= Duration::from_millis(500) {
            self.fps = self.fps_frames as f32 / now.duration_since(self.fps_stamp).as_secs_f32();
            self.fps_frames = 0;
            self.fps_stamp = now;
        }
        if now.duration_since(self.last_point) >= Duration::from_millis(100) {
            self.last_point = now;
            self.push_point();
        }
        if self.bench {
            let t = now.duration_since(self.started).as_secs_f32();
            if t > 1.0 {
                self.scroll_pos = (self.scroll_pos + 53) % ROWS;
                self.scroll.scroll_to_item(self.scroll_pos, ScrollStrategy::Top);
            }
            if t > 7.0 {
                let n = (self.frames - 1).max(1) as f64;
                let report = format!(
                    "frames={} avg_ms={:.2} max_ms={:.2} avg_fps={:.1} rows={ROWS} seconds={t:.1}\n",
                    self.frames,
                    self.frame_ms_sum / n,
                    self.frame_ms_max,
                    1000.0 / (self.frame_ms_sum / n)
                );
                let _ = std::fs::write("bench.txt", report);
                cx.quit();
            }
        }
        window.request_animation_frame();

        let avg = self.avg.clone();
        let p95 = self.p95.clone();
        let selected = self.selected;
        let stats = format!("{:.0} fps · {} frames · row {} selected", self.fps, self.frames, selected);

        let titlebar = div()
            .h(px(38.))
            .w_full()
            .flex()
            .items_center()
            .bg(rgb(PANEL))
            .border_b_1()
            .border_color(rgb(LINE))
            .child(div().pl(px(12.)).pr(px(14.)).text_color(rgb(ACCENT)).font_weight(gpui::FontWeight::SEMIBOLD).child("JMeter"))
            .child(div().flex().gap(px(12.)).text_color(rgb(MUTED)).text_size(px(12.5)).children(["File", "Edit", "Run", "View", "Tools", "Help"]))
            .child(div().flex_1().h_full().window_control_area(WindowControlArea::Drag))
            .child(div().w(px(360.)).child(self.palette.clone()))
            .child(div().flex_1().h_full().window_control_area(WindowControlArea::Drag))
            .child(Self::control("–", WindowControlArea::Min, PANEL2))
            .child(Self::control("□", WindowControlArea::Max, PANEL2))
            .child(Self::control("×", WindowControlArea::Close, 0x8a2f38));

        let sidebar = div()
            .w(px(270.))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(PANEL))
            .border_r_1()
            .border_color(rgb(LINE))
            .child(div().px(px(12.)).pt(px(9.)).pb(px(7.)).text_size(px(11.)).text_color(rgb(MUTED)).font_family("Lilex").child("TEST PLAN · 10,000 ROWS"))
            .child(
                uniform_list("rows", ROWS, cx.processor(|this: &mut Spike, range: Range<usize>, _w, cx| {
                    range.map(|ix| Spike::row(this.selected, ix, cx)).collect::<Vec<_>>()
                }))
                .track_scroll(&self.scroll)
                .flex_1(),
            );

        let chart = canvas(
            |b, _, _| b,
            move |bounds, _, window, _| {
                for i in 1..4 {
                    let y = bounds.top() + bounds.size.height * (i as f32 / 4.0);
                    window.paint_quad(fill(Bounds::new(point(bounds.left(), y), size(bounds.size.width, px(1.))), rgb(LINE)));
                }
                Spike::series_path(&p95, bounds, WARN, window);
                Spike::series_path(&avg, bounds, ACCENT, window);
            },
        )
        .w_full()
        .h(px(150.));

        let field = |label: &'static str, input: Entity<TextInput>| {
            div().flex().flex_col().gap(px(4.)).child(div().text_size(px(11.)).font_family("Lilex").text_color(rgb(MUTED)).child(label)).child(input)
        };

        let editor = div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div().flex().bg(rgb(PANEL)).border_b_1().border_color(rgb(LINE)).child(
                    div().px(px(14.)).py(px(8.)).bg(rgb(BG)).border_t_2().border_color(rgb(ACCENT)).child("POST /api/login"),
                ),
            )
            .child(
                div()
                    .p(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(div().text_size(px(19.)).font_weight(gpui::FontWeight::SEMIBOLD).child("POST /api/login"))
                    .child(field("NAME", self.name.clone()))
                    .child(field("PATH", self.url.clone()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .p(px(10.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(rgb(LINE))
                            .bg(rgb(PANEL))
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .text_size(px(11.))
                                    .font_family("Lilex")
                                    .text_color(rgb(MUTED))
                                    .child("RESPONSE TIME")
                                    .child(div().flex().gap(px(10.)).child(div().text_color(rgb(ACCENT)).child("— avg")).child(div().text_color(rgb(WARN)).child("— p95"))),
                            )
                            .child(chart),
                    )
                    .child(div().text_color(rgb(ERR)).text_size(px(12.)).child("Error sample: 500 · payment_gateway_timeout")),
            );

        let statusbar = div()
            .h(px(24.))
            .w_full()
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(16.))
            .bg(rgb(ACCENT))
            .text_color(rgb(0x0b1220))
            .text_size(px(11.5))
            .child("● Ready")
            .child(stats)
            .child(div().ml_auto().child(format!("name: {} · path: {}", self.name.read(cx).text(), self.url.read(cx).text())));

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(FG))
            .font_family("IBM Plex Sans")
            .text_size(px(13.))
            .child(titlebar)
            .child(div().flex_1().flex().min_h_0().child(sidebar).child(editor))
            .child(statusbar)
    }
}

fn load_fonts(cx: &App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/fonts/lilex/Lilex-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../../../assets/fonts/lilex/Lilex-Bold.ttf")),
    ];
    cx.text_system().add_fonts(fonts).expect("fonts load");
}

fn main() {
    let bench = std::env::args().any(|a| a == "--bench");
    application().run(move |cx: &mut App| {
        load_fonts(cx);
        text_input::bind_keys(cx);
        let bounds = Bounds::centered(None, size(px(1100.), px(680.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions { title: Some("JMeter UI spike".into()), appears_transparent: true, traffic_light_position: None }),
                window_min_size: Some(size(px(800.), px(500.))),
                ..Default::default()
            },
            move |window, cx| {
                let palette = cx.new(|cx| TextInput::new(cx, "Search elements, actions, variables", ""));
                let name = cx.new(|cx| TextInput::new(cx, "Name", "POST /api/login"));
                let url = cx.new(|cx| TextInput::new(cx, "Path", "/api/login"));
                window.focus(&name.read(cx).focus_handle_clone(), cx);
                cx.new(|_| {
                    let now = Instant::now();
                    let mut s = Spike {
                        palette,
                        name,
                        url,
                        scroll: UniformListScrollHandle::new(),
                        selected: 0,
                        bench,
                        started: now,
                        last_frame: now,
                        frames: 0,
                        frame_ms_sum: 0.0,
                        frame_ms_max: 0.0,
                        scroll_pos: 0,
                        last_point: now,
                        avg: vec![0.4; 120],
                        p95: vec![0.9; 120],
                        seed: 12345,
                        fps_stamp: now,
                        fps_frames: 0,
                        fps: 0.0,
                    };
                    for _ in 0..120 {
                        s.push_point();
                    }
                    s
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
