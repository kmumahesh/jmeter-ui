//! The command palette overlay.

use crate::fields::{TextField, TextFieldEvent};
use crate::theme::{self, with_alpha};
use crate::ui_util::{highlight_runs, icon};
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, IntoElement, MouseButton, Render, ScrollHandle, SharedString, Window,
    div, prelude::*, px,
};
use plan_core::{NodeId, Plan};
use shell_core::actions::AppContext;
use shell_core::palette::{self, Input, Mode, Ranked, Target};
use std::sync::Arc;

/// What the palette needs to know about the app when it opens.
#[derive(Clone)]
pub struct Snapshot {
    pub context: AppContext,
    pub plan: Option<Arc<Plan>>,
    pub selected: Option<NodeId>,
    pub recent: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteEvent {
    Confirmed(Target),
}

pub const MAX_RESULTS: usize = 60;

pub struct Palette {
    field: Entity<TextField>,
    snapshot: Snapshot,
    results: Vec<Ranked>,
    selected: usize,
    scroll: ScrollHandle,
    /// Set when the user tries to confirm something unavailable.
    message: Option<String>,
}

impl EventEmitter<PaletteEvent> for Palette {}
impl EventEmitter<DismissEvent> for Palette {}

impl Palette {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, snapshot: Snapshot, initial: &str) -> Self {
        let field = cx.new(|cx| TextField::new(cx, "palette-input", palette::Mode::Everything.placeholder(), initial));
        cx.subscribe(&field, |this, _, e: &TextFieldEvent, cx| match e {
            TextFieldEvent::Changed => this.refresh(cx),
            TextFieldEvent::Submitted => this.confirm(cx),
            TextFieldEvent::Cancelled => cx.emit(DismissEvent),
            TextFieldEvent::Up => this.move_selection(-1, cx),
            TextFieldEvent::Down => this.move_selection(1, cx),
            TextFieldEvent::PageUp => this.move_selection(-8, cx),
            TextFieldEvent::PageDown => this.move_selection(8, cx),
            TextFieldEvent::Blurred => {}
        })
        .detach();
        let handle = field.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        let mut p = Palette { field, snapshot, results: vec![], selected: 0, scroll: ScrollHandle::new(), message: None };
        p.refresh(cx);
        p
    }

    pub fn query(&self, cx: &App) -> String {
        self.field.read(cx).text().to_string()
    }

    pub fn mode(&self, cx: &App) -> Mode {
        palette::parse_query(self.field.read(cx).text()).0
    }

    pub fn results(&self) -> &[Ranked] {
        &self.results
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        let input = Input { context: self.snapshot.context, plan: self.snapshot.plan.as_deref(), selected: self.snapshot.selected, recent_files: &self.snapshot.recent };
        self.results = palette::results(&input, &query, MAX_RESULTS);
        self.selected = 0;
        self.message = None;
        let mode = palette::parse_query(&query).0;
        self.field.update(cx, |f, cx| f.set_placeholder(mode.placeholder(), cx));
        self.scroll.scroll_to_item(0);
        cx.notify();
    }

    pub fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.results.is_empty() {
            return;
        }
        let last = self.results.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
        self.message = None;
        self.scroll_to_selected();
        cx.notify();
    }

    fn scroll_to_selected(&self) {
        // Children of the list are group headers and items, so find the selected item's child index.
        let mut child = 0;
        let mut last_group: Option<&str> = None;
        for (i, r) in self.results.iter().enumerate() {
            if last_group != Some(r.item.group.as_str()) {
                child += 1;
                last_group = Some(r.item.group.as_str());
            }
            if i == self.selected {
                break;
            }
            child += 1;
        }
        self.scroll.scroll_to_item(child);
    }

    pub fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.results.get(self.selected) else { return };
        if let Some(why) = &r.item.disabled {
            self.message = Some(why.clone());
            cx.notify();
            return;
        }
        let target = r.item.target.clone();
        cx.emit(PaletteEvent::Confirmed(target));
    }

    fn click(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        self.confirm(cx);
    }
}



impl Focusable for Palette {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.read(cx).focus_handle(cx)
    }
}

impl Render for Palette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let mode = self.mode(cx);
        let mode_icon = match mode {
            Mode::Add => "icons/plus.svg",
            Mode::Commands => "icons/chevron_right.svg",
            _ => "icons/magnifying_glass.svg",
        };

        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        let mut last_group: Option<&str> = None;
        for (i, r) in self.results.iter().enumerate() {
            if last_group != Some(r.item.group.as_str()) {
                last_group = Some(r.item.group.as_str());
                rows.push(
                    div()
                        .px(px(14.))
                        .pt(px(8.))
                        .pb(px(3.))
                        .text_size(px(10.5))
                        .font_family(crate::assets::MONO_FONT)
                        .text_color(p.muted)
                        .child(r.item.group.to_uppercase())
                        .into_any_element(),
                );
            }
            let disabled = r.item.disabled.is_some();
            let selected = i == self.selected;
            let title_color = if disabled { with_alpha(p.fg, 0.5) } else { p.fg };
            let title = highlight_runs(&r.item.title, &r.positions)
                .into_iter()
                .map(|(t, hit)| {
                    div()
                        .whitespace_nowrap()
                        .when(hit && !disabled, |d| d.text_color(p.accent).font_weight(FontWeight::SEMIBOLD))
                        .child(SharedString::from(t))
                })
                .collect::<Vec<_>>();
            let lead = match r.item.category {
                Some(c) => icon(c.icon(), 14., p.category(c)),
                None => icon("icons/chevron_right.svg", 14., p.muted),
            };
            let detail = r.item.disabled.clone().unwrap_or_else(|| r.item.detail.clone());
            rows.push(
                div()
                    .id(("palette-row", i))
                    .debug_selector(move || format!("palette-row-{i}"))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .h(px(28.))
                    .text_color(title_color)
                    .border_l_2()
                    .border_color(if selected { p.accent } else { with_alpha(p.accent, 0.0) })
                    .when(selected, |d| d.bg(p.selection()))
                    .when(!selected, |d| d.hover(|d| d.bg(p.panel2)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.click(i, cx)))
                    .child(lead)
                    .child(div().flex().children(title))
                    .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(12.)).text_color(p.muted).child(detail))
                    .when_some(r.item.shortcut, |d, s| {
                        d.child(div().px(px(6.)).rounded(px(3.)).border_1().border_color(p.line).text_size(px(10.5)).font_family(crate::assets::MONO_FONT).text_color(p.muted).child(s))
                    })
                    .into_any_element(),
            );
        }
        if self.results.is_empty() {
            rows.push(div().px(px(14.)).py(px(14.)).text_color(p.muted).child(empty_text(mode, self.snapshot.plan.is_some())).into_any_element());
        }

        div()
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_start()
            .pt(px(54.))
            .bg(with_alpha(gpui::black(), 0.35))
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            .child(
                div()
                    .id("palette")
                    .w(px(580.))
                    .max_w_full()
                    .flex()
                    .flex_col()
                    .bg(p.raise)
                    .border_1()
                    .border_color(p.line)
                    .rounded(px(10.))
                    .shadow_lg()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(12.))
                            .py(px(10.))
                            .border_b_1()
                            .border_color(p.line)
                            .child(icon(mode_icon, 16., p.muted))
                            .child(div().flex_1().child(self.field.clone())),
                    )
                    .child(div().id("palette-results").max_h(px(360.)).overflow_y_scroll().track_scroll(&self.scroll).children(rows))
                    .when_some(self.message.clone(), |d, m| d.child(div().px(px(14.)).py(px(6.)).bg(with_alpha(p.warn, 0.15)).text_color(p.warn).text_size(px(12.)).child(m)))
                    .child(
                        div()
                            .flex()
                            .gap(px(16.))
                            .px(px(14.))
                            .py(px(7.))
                            .border_t_1()
                            .border_color(p.line)
                            .bg(p.panel)
                            .text_size(px(11.5))
                            .text_color(p.muted)
                            .child("\u{2191}\u{2193} navigate")
                            .child("Enter select")
                            .child("Esc close")
                            .child(div().ml_auto().child("> commands   @ elements   + add")),
                    ),
            )
    }
}

fn empty_text(mode: Mode, has_plan: bool) -> &'static str {
    match (mode, has_plan) {
        (Mode::Elements | Mode::Add, false) => "Open a plan first",
        (Mode::Add, true) => "Nothing can be added under the selected element",
        _ => "No matches",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_messages() {
        assert_eq!(empty_text(Mode::Elements, false), "Open a plan first");
        assert_eq!(empty_text(Mode::Add, true), "Nothing can be added under the selected element");
        assert_eq!(empty_text(Mode::Commands, true), "No matches");
    }
}
