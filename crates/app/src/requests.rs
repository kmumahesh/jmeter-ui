//! The request viewer: what was sent and what came back for the requests of a run, as a list with one request
//! opened beside it. The details are kept by a listener inside JMeter (see `jvm_runtime::details`); this module
//! shows them, for a live run as they arrive and for a stored run read from its folder.

use crate::assets::MONO_FONT;
use crate::shell::{BottomTab, RequestPart, Shell};
use crate::theme::{self, Palette};
use gpui::{AnyElement, Context, FontWeight, IntoElement, Window, div, prelude::*, px};
use jvm_runtime::details::Detail;
use shell_core::settings::RequestDetails;

/// The height the lower panel is given for the viewer, so a list and a request fit side by side.
pub const REQUESTS_PANEL_HEIGHT: f32 = 420.0;

/// Rows drawn in the list (the run keeps no more than this many anyway).
const MAX_LIST_ROWS: usize = 500;

/// The most characters of a label shown in the list.
const LABEL_CHARS: usize = 60;

fn short(label: &str) -> String {
    if label.chars().count() > LABEL_CHARS {
        let mut s: String = label.chars().take(LABEL_CHARS - 1).collect();
        s.push('\u{2026}');
        s
    } else {
        label.to_string()
    }
}

impl Shell {
    /// Show the request viewer, with the request at `pick` (an index into the run's details) opened.
    pub fn show_requests(&mut self, pick: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.open_requests(pick, cx);
        // Asked for, so the arrow keys work at once.
        window.focus(&self.req_focus, cx);
    }

    /// The same without taking the keyboard, for when the viewer opens by itself.
    pub fn open_requests(&mut self, pick: Option<usize>, cx: &mut Context<Self>) {
        if self.run.is_none() {
            return;
        }
        self.bottom_tab = BottomTab::Requests;
        if !self.workspace.bottom.visible {
            self.workspace.toggle_bottom();
        }
        if self.workspace.bottom.size < REQUESTS_PANEL_HEIGHT {
            self.workspace.bottom.size = REQUESTS_PANEL_HEIGHT;
        }
        if let Some(i) = pick {
            self.select_request(i, cx);
        } else {
            self.sync_request_texts(cx);
        }
        cx.notify();
    }

    /// Open the request at `i` (clamped to the details there are).
    pub fn select_request(&mut self, i: usize, cx: &mut Context<Self>) {
        let n = self.run.as_ref().map_or(0, |r| r.details.len());
        self.req_sel = if n == 0 { 0 } else { i.min(n - 1) };
        self.sync_request_texts(cx);
        cx.notify();
    }

    /// Move the chosen request up or down the list.
    pub fn step_request(&mut self, down: bool, cx: &mut Context<Self>) {
        let n = self.run.as_ref().map_or(0, |r| r.details.len());
        if n == 0 {
            return;
        }
        let next = if down { (self.req_sel + 1).min(n - 1) } else { self.req_sel.saturating_sub(1) };
        if next != self.req_sel {
            self.select_request(next, cx);
        }
    }

    /// Put the text of the chosen request into the two read-only boxes.
    pub fn sync_request_texts(&mut self, cx: &mut Context<Self>) {
        let (req, resp) = match self.run.as_ref().and_then(|r| r.details.get(self.req_sel)) {
            Some(d) => (d.request_text(), d.response_text()),
            None => (String::new(), String::new()),
        };
        self.req_request.update(cx, |a, cx| {
            if a.text() != req {
                a.set_text(&req, cx);
            }
        });
        self.req_response.update(cx, |a, cx| {
            if a.text() != resp {
                a.set_text(&resp, cx);
            }
        });
    }

    /// When the viewer goes away (another tab, a hidden panel, a new run) while it holds the keyboard, the keyboard goes
    /// back to the window, so the shortcuts keep working.
    pub(crate) fn give_back_viewer_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let showing = self.bottom_tab == BottomTab::Requests && self.workspace.bottom.visible;
        if showing {
            return;
        }
        let held = self.req_focus.is_focused(window) || self.req_request.read(cx).is_focused(window) || self.req_response.read(cx).is_focused(window);
        if held {
            window.focus(&self.focus, cx);
        }
    }

    /// Choose which requests keep their details (Settings).
    pub fn set_request_details(&mut self, v: RequestDetails, cx: &mut Context<Self>) {
        self.settings.request_details = v;
        self.save_settings();
        cx.notify();
    }

    /// Open the viewer on the first failed request of the run, or the first request when none failed.
    pub fn show_first_failure(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.run.as_ref().and_then(|r| r.details.iter().position(|d| !d.success));
        self.show_requests(Some(at.unwrap_or(0)), window, cx);
    }

    // ---- drawing ----

    /// The Requests tab of the lower panel.
    pub(crate) fn requests_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let Some(run) = &self.run else {
            return div().p(px(16.)).text_color(p.muted).child("Run the plan to see what was sent and what came back.").into_any_element();
        };
        if run.details.is_empty() {
            return self.requests_empty(run, &p);
        }
        let sel = self.req_sel.min(run.details.len() - 1);
        let list = div()
            .id("req-list")
            .debug_selector(|| "req-list".to_string())
            .w(px(300.))
            .flex_none()
            .h_full()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(p.line)
            .children(run.details.iter().take(MAX_LIST_ROWS).enumerate().map(|(i, d)| {
                let on = i == sel;
                div()
                    .id(("req-row", i))
                    .debug_selector(move || format!("req-row-{i}"))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(3.))
                    .text_size(px(12.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(p.panel2))
                    .hover(|d| d.bg(p.panel2))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        window.focus(&this.req_focus, cx);
                        this.select_request(i, cx)
                    }))
                    .child(div().size(px(8.)).flex_none().rounded_full().bg(if d.success { p.ok } else { p.err }))
                    .child(div().w(px(34.)).flex_none().font_family(MONO_FONT).text_color(if d.success { p.muted } else { p.err }).child(d.code.clone()))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().child(short(&d.label)))
                    .child(div().flex_none().font_family(MONO_FONT).text_color(p.muted).child(format!("{} ms", d.elapsed)))
            }));
        let d = &run.details[sel];
        let part_tab = |id: &'static str, label: String, which: RequestPart, cx: &mut Context<Self>| {
            let active = self.req_part == which;
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .px(px(10.))
                .py(px(4.))
                .text_size(px(12.))
                .cursor_pointer()
                .when(active, |d| d.text_color(p.fg).border_b_2().border_color(p.accent))
                .when(!active, |d| d.text_color(p.muted).hover(|d| d.text_color(p.fg)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.req_part = which;
                    cx.notify();
                }))
                .child(label)
        };
        let failed_checks = d.assertions.iter().filter(|a| a.failure || a.error).count();
        let body: AnyElement = match self.req_part {
            RequestPart::Request => div().debug_selector(|| "req-request".to_string()).child(self.req_request.clone()).into_any_element(),
            RequestPart::Response => div().debug_selector(|| "req-response".to_string()).child(self.req_response.clone()).into_any_element(),
            RequestPart::Checks => self.request_checks(d, &p),
        };
        let note = if d.truncated {
            Some("This text was cut at 64 KB.")
        } else {
            None
        };
        div()
            .id("requests")
            .debug_selector(|| "requests".to_string())
            .track_focus(&self.req_focus)
            .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, _, cx| match ev.keystroke.key.as_str() {
                "down" => this.step_request(true, cx),
                "up" => this.step_request(false, cx),
                _ => {}
            }))
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(4.))
                    .border_b_1()
                    .border_color(p.line)
                    .text_size(px(11.5))
                    .text_color(p.muted)
                    .child(div().debug_selector(|| "req-count".to_string()).child(format!("{} request{} kept", run.details.len(), if run.details.len() == 1 { "" } else { "s" })))
                    .when(run.details_skipped > 0, |d| d.child(format!("\u{b7} {} could not be read", run.details_skipped)))
                    .when(run.details.len() >= jvm_runtime::details::MAX_KEPT as usize, |d| d.child(format!("\u{b7} only the first {} are kept", jvm_runtime::details::MAX_KEPT))),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(list)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .px(px(12.))
                                    .py(px(4.))
                                    .flex()
                                    .gap(px(10.))
                                    .items_center()
                                    .child(div().id("req-title").debug_selector(|| "req-title".to_string()).font_weight(FontWeight::SEMIBOLD).overflow_hidden().whitespace_nowrap().child(short(&d.label)))
                                    .child(div().font_family(MONO_FONT).text_color(if d.success { p.ok } else { p.err }).child(format!("{} {}", d.code, d.message).trim().to_string()))
                                    .child(div().font_family(MONO_FONT).text_color(p.muted).child(format!("{} ms \u{b7} latency {} ms \u{b7} connect {} ms \u{b7} {} bytes", d.elapsed, d.latency, d.connect, d.bytes))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .border_b_1()
                                    .border_color(p.line)
                                    .child(part_tab("req-tab-request", "Request".into(), RequestPart::Request, cx))
                                    .child(part_tab("req-tab-response", "Response".into(), RequestPart::Response, cx))
                                    .child(part_tab("req-tab-checks", if failed_checks > 0 { format!("Checks ({failed_checks} failed)") } else { format!("Checks ({})", d.assertions.len()) }, RequestPart::Checks, cx)),
                            )
                            .when_some(note, |x, n| x.child(div().px(px(12.)).py(px(2.)).text_size(px(11.5)).text_color(p.warn).child(n)))
                            .child(div().id("req-body").flex_1().min_h_0().overflow_y_scroll().p(px(8.)).child(body)),
                    ),
            )
            .into_any_element()
    }

    fn requests_empty(&self, run: &crate::running::RunSession, p: &Palette) -> AnyElement {
        let (title, hint) = if run.details_off {
            ("Requests and responses are not kept", "Turn them on in Settings, under Request details, then run again.")
        } else if run.phase.active() {
            ("Waiting for requests", "Failed requests show up here as they happen. Choose All requests in Settings to keep every request.")
        } else if self.settings.request_details == RequestDetails::Failed && run.kind == jvm_runtime::runs::RunKind::Run {
            ("No request failed", "Only failed requests are kept by default. Choose All requests in Settings to look at the others.")
        } else {
            ("No requests were kept for this run", "Runs made before this view existed, and runs that sent nothing, have none.")
        };
        div()
            .id("requests-empty")
            .debug_selector(|| "requests-empty".to_string())
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .text_color(p.muted)
            .child(div().text_color(p.fg).child(title))
            .child(hint)
            .when(run.details_skipped > 0, |d| d.child(format!("{} lines of the details file could not be read.", run.details_skipped)))
            .into_any_element()
    }

    fn request_checks(&self, d: &Detail, p: &Palette) -> AnyElement {
        let mut col = div().flex().flex_col().gap(px(3.)).text_size(px(12.)).id("req-checks").debug_selector(|| "req-checks".to_string());
        if d.assertions.is_empty() && d.sub_results.is_empty() {
            col = col.child(div().text_color(p.muted).child("This request has no assertions, and no other requests were made as part of it."));
        }
        for a in &d.assertions {
            let bad = a.failure || a.error;
            col = col.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(div().w(px(44.)).flex_none().text_color(if bad { p.err } else { p.ok }).child(if bad { "Failed" } else { "Passed" }))
                    .child(div().child(a.name.clone()))
                    .when(!a.message.is_empty(), |x| x.child(div().text_color(p.muted).child(a.message.clone()))),
            );
        }
        if !d.sub_results.is_empty() {
            col = col.child(div().pt(px(6.)).text_color(p.muted).child("Made as part of this request"));
            for s in &d.sub_results {
                col = col.child(div().flex().gap(px(8.)).font_family(MONO_FONT).child(div().w(px(44.)).flex_none().text_color(if s.ok { p.muted } else { p.err }).child(s.code.clone())).child(div().child(short(&s.label))).child(div().text_color(p.muted).child(format!("{} ms", s.ms))));
            }
        }
        col.into_any_element()
    }
}
