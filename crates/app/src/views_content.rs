//! The content of tabs and panels: the plan editor, settings, inspector, results and status bar.

use crate::assets::MONO_FONT;
use crate::shell::{BottomTab, Dialog, LoadedPlan, Notice, Severity, Shell, Then};
use crate::theme::{self, with_alpha};
use crate::ui_util::{chip, icon, section_title};
use gpui::{AnyElement, Context, FontWeight, IntoElement, MouseButton, MouseDownEvent, Window, div, prelude::*, px};
use plan_core::problems::{self, Level};
use plan_core::{Category, catalog, rules};
use shell_core::actions::Action;
use shell_core::layout::{Tab, TabKind};
use shell_core::settings::{RequestDetails, ThemeChoice};
use ui::{Button, ButtonCommon, ButtonSize, ButtonStyle, Clickable, Disableable, IconButton, IconName, IconSize, Tooltip};

impl Shell {
    pub(crate) fn tab_content(&self, tab: &Tab, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match tab.kind {
            TabKind::Plan(id) => self.plan_editor(id, cx),
            TabKind::Settings => self.settings_view(cx),
        }
    }

    // ---- welcome ----

    pub(crate) fn welcome(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let recents = self.settings.recent_files.clone();
        div()
            .id("welcome")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(70.))
            .gap(px(14.))
            .child(icon("icons/jm/zap.svg", 44., p.accent))
            .child(div().text_size(px(24.)).font_weight(FontWeight::SEMIBOLD).child("JMeter UI"))
            .child(div().text_color(p.muted).child("Open a test plan to see its elements, or press Ctrl+K for every command."))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(div().debug_selector(|| "welcome-new".to_string()).child(Button::new("welcome-new", "New Plan\u{2026}").style(ButtonStyle::Tinted(ui::TintColor::Accent)).on_click(cx.listener(|this, _, window, cx| this.perform(Action::NewPlan, window, cx)))))
                    .child(div().debug_selector(|| "welcome-open".to_string()).child(Button::new("welcome-open", "Open Plan\u{2026}").style(ButtonStyle::Subtle).on_click(cx.listener(|this, _, window, cx| this.perform(Action::OpenPlan, window, cx))))),
            )
            .when(!recents.is_empty(), |d| {
                d.child(
                    div()
                        .mt(px(14.))
                        .w(px(460.))
                        .max_w_full()
                        .flex()
                        .flex_col()
                        .child(div().pb(px(6.)).text_size(px(11.)).font_family(MONO_FONT).text_color(p.muted).child("RECENT PLANS"))
                        .children(recents.iter().take(6).enumerate().map(|(i, path)| {
                            let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
                            let open_path = path.clone();
                            div()
                                .id(("recent", i))
                                .flex()
                                .items_baseline()
                                .gap(px(10.))
                                .px(px(8.))
                                .py(px(4.))
                                .rounded(px(5.))
                                .cursor_pointer()
                                .hover(|d| d.bg(p.panel2))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_plan_path(std::path::Path::new(&open_path), window, cx);
                                }))
                                .child(div().text_color(p.accent).child(name))
                                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(11.5)).text_color(p.muted).child(path.clone()))
                        })),
                )
            })
            .into_any_element()
    }

    /// What the plan holds, shown with the Test Plan element at the top of the tree.
    pub(crate) fn plan_summary(&self, lp: &LoadedPlan, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let summary = lp.plan.summary();
        div()
            .id("plan-summary")
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().font_family(MONO_FONT).text_size(px(11.5)).text_color(p.muted).child(crate::shell::display_path(&lp.path)))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .children(summary.by_category.iter().filter(|(c, _)| !matches!(c, Category::TestPlan)).map(|(c, n)| chip(format!("{n} {}", if *n == 1 { c.label().to_string() } else { c.plural().to_string() }), p.category(*c)))),
            )
            .child(div().text_color(p.muted).child(format!("{} elements in total.", summary.total)))
            .into_any_element()
    }

    // ---- settings ----

    fn settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let Some(f) = &self.settings_fields else { return div().into_any_element() };
        let current = self.settings.theme;
        let seg = |id: &'static str, label: &'static str, choice: ThemeChoice, cx: &mut Context<Self>| {
            div().debug_selector(move || id.to_string()).child(
                Button::new(id, label)
                    .style(if current == choice { ButtonStyle::Filled } else { ButtonStyle::Subtle })
                    .size(ButtonSize::Default)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_theme(choice, cx))),
            )
        };
        let status_line = |ok: bool, text: String| {
            div().flex().gap(px(8.)).items_center().text_size(px(12.5)).child(icon(if ok { "icons/check.svg" } else { "icons/warning.svg" }, 14., if ok { p.ok } else { p.warn })).child(text)
        };
        let java_line = match (&self.runtime.java, &self.runtime.problem) {
            (Some(j), _) => status_line(true, j.clone()),
            (None, Some(e)) => status_line(false, e.clone()),
            _ => status_line(false, "Not checked yet".into()),
        };
        let jmeter_line = match (&self.runtime.jmeter, self.settings.jmeter_home.is_some()) {
            (Some(v), _) => status_line(true, format!("JMeter {v}")),
            (None, true) => status_line(false, "JMeter was not found in that folder".into()),
            (None, false) => status_line(false, "No JMeter folder chosen".into()),
        };
        let heading = |t: &str| div().text_size(px(11.)).font_family(MONO_FONT).text_color(p.muted).child(t.to_string());
        let labelled = |label: &str, field: AnyElement, error: Option<String>, reserve_error_line: bool| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(div().text_size(px(12.)).text_color(p.muted).child(label.to_string()))
                .child(field)
                .when(reserve_error_line, |d| d.child(div().h(px(16.)).text_size(px(11.5)).text_color(p.err).child(error.unwrap_or_default())))
        };
        let heap_error = f.heap_mb.read(cx).problem().map(|e| e.message());
        let interval_error = f.summary_secs.read(cx).problem().map(|e| e.message());
        div()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(20.))
            .max_w(px(720.))
            .child(div().text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child("Settings"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(heading("COLOR THEME"))
                    .child(div().flex().gap(px(6.)).child(seg("theme-system", "Follow system", ThemeChoice::System, cx)).child(seg("theme-dark", "Dark", ThemeChoice::Dark, cx)).child(seg("theme-light", "Light", ThemeChoice::Light, cx))),
            )
            .child(div().flex().flex_col().gap(px(8.)).child(heading("ON STARTUP")).child(div().flex().child(f.startup.clone())))
            .child(self.request_details_section(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(heading("RUNTIME"))
                    .child(div().text_color(p.muted).text_size(px(12.5)).child("The app starts JMeter in non-GUI mode using this Java and this JMeter folder."))
                    .child(labelled("JMeter folder", f.jmeter_home.clone().into_any_element(), None, false))
                    .child(labelled("Java executable", f.java_path.clone().into_any_element(), None, false))
                    .child(div().flex().gap(px(16.)).child(div().w(px(200.)).child(labelled("Heap size (MB)", f.heap_mb.clone().into_any_element(), heap_error, true))).child(div().w(px(200.)).child(labelled("Summary every (seconds)", f.summary_secs.clone().into_any_element(), interval_error, true))))
                    .child(div().flex().gap(px(8.)).child(div().debug_selector(|| "apply-runtime".to_string()).child(Button::new("apply-runtime", "Apply and check").style(ButtonStyle::Tinted(ui::TintColor::Accent)).on_click(cx.listener(|this, _, window, cx| this.apply_runtime_settings(window, cx))))))
                    .child(java_line)
                    .child(jmeter_line),
            )
            .child(self.runs_settings_section(cx))
            .child(
                div().flex().flex_col().gap(px(4.)).child(heading("SETTINGS FILE")).child(
                    div().font_family(MONO_FONT).text_size(px(11.5)).text_color(p.muted).child(self.settings_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "Not saved (no settings path)".into())),
                ),
            )
            .into_any_element()
    }

    /// Settings > Request details: which requests keep what was sent and received.
    fn request_details_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let current = self.settings.request_details;
        let seg = |id: &'static str, label: &'static str, choice: RequestDetails, cx: &mut Context<Self>| {
            div().debug_selector(move || id.to_string()).child(
                Button::new(id, label)
                    .style(if current == choice { ButtonStyle::Filled } else { ButtonStyle::Subtle })
                    .size(ButtonSize::Default)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_request_details(choice, cx))),
            )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().text_size(px(11.)).font_family(MONO_FONT).text_color(p.muted).child("REQUEST DETAILS"))
            .child(div().text_color(p.muted).text_size(px(12.5)).child("What was sent and what came back, for the Requests tab. A run keeps at most 500 requests. Checking a plan or sending one request always keeps them."))
            .child(div().flex().gap(px(6.)).child(seg("details-failed", "Failed requests", RequestDetails::Failed, cx)).child(seg("details-all", "All requests", RequestDetails::All, cx)).child(seg("details-off", "Off", RequestDetails::Off, cx)))
            .into_any_element()
    }

    /// Settings > Run history: where run files go, how many runs are kept, and clearing the shared folder.
    fn runs_settings_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let Some(f) = &self.settings_fields else { return div().into_any_element() };
        let custom = self.settings.runs_custom;
        let heading = |t: &str| div().text_size(px(11.)).font_family(MONO_FONT).text_color(p.muted).child(t.to_string());
        let seg = |id: &'static str, label: &'static str, on: bool, custom_choice: bool, cx: &mut Context<Self>| {
            div().debug_selector(move || id.to_string()).child(
                Button::new(id, label)
                    .style(if on { ButtonStyle::Filled } else { ButtonStyle::Subtle })
                    .size(ButtonSize::Default)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_runs_custom(custom_choice, cx))),
            )
        };
        let folder = self.settings.runs_folder.clone();
        let shown = self.shared_runs_root().map(|r| r.display().to_string());
        let keep_error = f.runs_keep.read(cx).problem().map(|e| e.message());
        div()
            .id("settings-runs")
            .debug_selector(|| "settings-runs".to_string())
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(heading("RUN HISTORY"))
            .child(div().text_color(p.muted).text_size(px(12.5)).child("Every run gets its own folder inside a folder named runs, with its results, log and the plan it ran. Pick where that runs folder goes."))
            .child(div().flex().gap(px(6.)).child(seg("runs-plan-folder", "Same folder as the .jmx file", !custom, false, cx)).child(seg("runs-custom-folder", "Custom folder", custom, true, cx)))
            .when(custom, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .id("runs-folder-path")
                                .debug_selector(|| "runs-folder-path".to_string())
                                .flex_1()
                                .px(px(8.))
                                .py(px(5.))
                                .rounded(px(5.))
                                .border_1()
                                .border_color(p.line)
                                .bg(p.panel2)
                                .font_family(MONO_FONT)
                                .text_size(px(11.5))
                                .text_color(if folder.is_some() { p.fg } else { p.muted })
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(folder.clone().unwrap_or_else(|| "No folder chosen: runs go next to the .jmx file until you choose one".into())),
                        )
                        .child(div().debug_selector(|| "runs-choose-folder".to_string()).child(Button::new("runs-choose-folder", "Choose folder\u{2026}").style(ButtonStyle::Tinted(ui::TintColor::Accent)).on_click(cx.listener(|this, _, window, cx| this.choose_runs_folder(window, cx, None))))),
                )
                .child(div().text_color(p.muted).text_size(px(12.)).child(match &shown {
                    Some(r) => format!("Run files are kept in {r}, shared by all plans."),
                    None => "Run files are kept next to each .jmx file.".to_string(),
                }))
            })
            .when(!custom, |d| d.child(div().text_color(p.muted).text_size(px(12.)).child("Run files are kept in a runs folder next to each .jmx file.")))
            .child(
                div().flex().items_start().gap(px(16.)).child(div().w(px(200.)).flex().flex_col().gap(px(4.)).child(div().text_size(px(12.)).text_color(p.muted).child("Keep the last runs of each plan")).child(div().debug_selector(|| "settings-runs-keep".to_string()).child(f.runs_keep.clone())).child(div().h(px(16.)).text_size(px(11.5)).text_color(p.err).child(keep_error.unwrap_or_default()))),
            )
            .child(
                div().flex().gap(px(8.)).child(
                    div().debug_selector(|| "runs-clear".to_string()).child(
                        Button::new("runs-clear", "Clear runs folder\u{2026}")
                            .style(ButtonStyle::Subtle)
                            .disabled(!Action::ClearRunsFolder.availability(&self.context()).is_enabled())
                            .tooltip(Tooltip::text("Removes the runs in the custom folder only; runs next to a .jmx file are never touched"))
                            .on_click(cx.listener(|this, _, window, cx| this.perform(Action::ClearRunsFolder, window, cx))),
                    ),
                ),
            )
            .into_any_element()
    }

    // ---- inspector: scope and order, and what can be added ----

    pub(crate) fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let mut body = div().flex().flex_col().pb(px(12.));
        match self.plan() {
            None => {
                body = body.child(div().p(px(14.)).text_color(p.muted).text_size(px(12.5)).child("Open a plan to see what can be added to the selected element."));
            }
            Some(lp) => {
                let sel = lp.tree.selected().or_else(|| lp.plan.roots().first().copied());
                if let Some(n) = sel.and_then(|s| lp.plan.node(s)) {
                    body = body.child(self.scope_section(lp, n.id, cx));
                    body = body.child(section_title("Can add", cx));
                    let types: Vec<_> = catalog::addable().into_iter().filter(|t| rules::can_add(n.category, t.category)).collect();
                    if types.is_empty() {
                        body = body.child(div().px(px(12.)).text_color(p.muted).text_size(px(12.5)).child("Nothing can be added under this element."));
                    }
                    let mut last = None;
                    for (i, t) in types.into_iter().enumerate() {
                        if last != Some(t.category) {
                            last = Some(t.category);
                            body = body.child(div().px(px(12.)).pt(px(8.)).pb(px(2.)).flex().items_center().gap(px(6.)).text_size(px(11.)).text_color(p.muted).child(icon(t.category.icon(), 13., p.category(t.category))).child(t.category.plural().to_string()));
                        }
                        let label = t.label;
                        body = body.child(
                            div()
                                .id(("can-add", i))
                                .debug_selector(move || format!("can-add-{label}"))
                                .px(px(12.))
                                .pl(px(31.))
                                .py(px(2.))
                                .text_size(px(12.5))
                                .cursor_pointer()
                                .hover(|d| d.bg(p.panel2))
                                .on_click(cx.listener(move |this, _, window, cx| this.add_element(t.tag, t.gui.unwrap_or_default(), window, cx)))
                                .child(label),
                        );
                    }
                }
            }
        }
        div()
            .id("inspector")
            .debug_selector(|| "inspector".to_string())
            .flex_none()
            .w(px(self.workspace.inspector.size))
            .h_full()
            .bg(p.panel)
            .overflow_y_scroll()
            .child(div().flex().items_center().justify_between().px(px(12.)).pt(px(8.)).pb(px(4.)).child(div().text_size(px(11.)).font_family(MONO_FONT).text_color(p.muted).child("INSPECTOR")))
            .child(body)
    }

    /// Which elements apply to the selected one, or which it applies to, and in what order they run.
    fn scope_section(&self, lp: &LoadedPlan, id: plan_core::NodeId, cx: &mut Context<Self>) -> AnyElement {
        use plan_core::scope::{self, Phase};
        let p = theme::palette(cx);
        let Some(n) = lp.plan.node(id) else { return div().into_any_element() };
        let name = |x: plan_core::NodeId| lp.plan.node(x).map(|n| n.name.clone()).unwrap_or_default();
        let row = |key: String, node: plan_core::NodeId, detail: String, active: bool, cx: &mut Context<Self>| {
            let nn = lp.plan.node(node);
            let color = nn.map(|n| p.category(n.category)).unwrap_or(p.muted);
            let icon_path = nn.map(|n| n.category.icon()).unwrap_or("icons/jm/dot.svg");
            let label = name(node);
            div()
                .id(gpui::SharedString::from(key.clone()))
                .debug_selector(move || key.clone())
                .flex()
                .items_center()
                .gap(px(6.))
                .px(px(12.))
                .py(px(2.))
                .text_size(px(12.5))
                .cursor_pointer()
                .hover(|d| d.bg(p.panel2))
                .when(!active, |d| d.opacity(0.5))
                .on_click(cx.listener(move |this, _, window, cx| this.select_node(node, window, cx)))
                .child(icon(icon_path, 14., color))
                .child(div().flex_none().when(!active, |d| d.line_through()).child(label))
                .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_size(px(11.)).text_color(p.muted).child(detail))
                .into_any_element()
        };
        let mut out = div().flex().flex_col().child(section_title("Scope & Order", cx));
        match n.category {
            Category::Sampler => {
                let entries = scope::for_sampler(&lp.plan, id);
                if entries.is_empty() {
                    return out.into_any_element();
                }
                let mut last: Option<Phase> = None;
                let mut step = 0;
                for (i, e) in entries.iter().enumerate() {
                    if last != Some(e.phase) {
                        last = Some(e.phase);
                        step += 1;
                        let note = e.phase.note();
                        out = out.child(
                            div()
                                .px(px(12.))
                                .pt(px(6.))
                                .pb(px(1.))
                                .flex()
                                .gap(px(6.))
                                .text_size(px(11.))
                                .text_color(p.muted)
                                .child(format!("{step}. {}", e.phase.label()))
                                .when(!note.is_empty(), |d| d.child(div().text_color(with_alpha(p.muted, 0.7)).child(format!("({note})")))),
                        );
                    }
                    let detail = if e.node == id { "this sampler".to_string() } else if e.from == id { "inside it".to_string() } else { format!("in {}", name(e.from)) };
                    out = out.child(row(format!("scope-{}", e.node.0), e.node, detail, e.active, cx));
                    let _ = i;
                }
            }
            c if scope::applies_to(&lp.plan, id).is_empty() && !matches!(c, Category::Controller | Category::ThreadGroup | Category::TestPlan | Category::WorkBench | Category::Fragment) => {
                return div().into_any_element();
            }
            Category::Controller | Category::ThreadGroup | Category::TestPlan | Category::WorkBench | Category::Fragment => {
                let shared = scope::shared_by_children(&lp.plan, id);
                if shared.is_empty() {
                    return out.child(div().px(px(12.)).text_size(px(12.)).text_color(p.muted).child("Nothing here applies to the elements inside it.")).into_any_element();
                }
                out = out.child(div().px(px(12.)).pb(px(2.)).text_size(px(11.)).text_color(p.muted).child("Apply to every sampler inside this element"));
                for e in &shared {
                    out = out.child(row(format!("scope-{}", e.node.0), e.node, e.phase.label().to_string(), e.active, cx));
                }
            }
            _ => {
                let targets = scope::applies_to(&lp.plan, id);
                out = out.child(div().px(px(12.)).pb(px(2.)).text_size(px(11.)).text_color(p.muted).child(format!("Applies to {} sampler{}", targets.len(), if targets.len() == 1 { "" } else { "s" })));
                const SHOWN: usize = 40;
                for t in targets.iter().take(SHOWN) {
                    out = out.child(row(format!("scope-{}", t.0), *t, String::new(), true, cx));
                }
                if targets.len() > SHOWN {
                    out = out.child(div().px(px(12.)).text_size(px(11.5)).text_color(p.muted).child(format!("and {} more", targets.len() - SHOWN)));
                }
            }
        }
        out.into_any_element()
    }

    // ---- results panel ----

    pub(crate) fn bottom_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let problems: Vec<problems::Problem> = self.plan().map(|lp| lp.problems().to_vec()).unwrap_or_default();
        let tab = |id: &'static str, label: String, which: BottomTab, cx: &mut Context<Self>| {
            let active = self.bottom_tab == which;
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .px(px(12.))
                .py(px(6.))
                .text_size(px(12.5))
                .cursor_pointer()
                .when(active, |d| d.text_color(p.fg).border_b_2().border_color(p.accent))
                .when(!active, |d| d.text_color(p.muted).hover(|d| d.text_color(p.fg)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.bottom_tab = which;
                    if which == BottomTab::History {
                        this.refresh_history();
                    }
                    cx.notify();
                }))
                .child(label)
        };
        let content: AnyElement = match self.bottom_tab {
            BottomTab::Run => self.run_view(cx),
            BottomTab::History => self.history_view(cx),
            BottomTab::Requests => self.requests_view(cx),
            BottomTab::Problems => {
                if problems.is_empty() {
                    div().p(px(16.)).text_color(p.muted).child(if self.plan().is_some() { "No problems found in this plan." } else { "Open a plan to check it." }).into_any_element()
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .children(problems.iter().enumerate().map(|(i, pr)| {
                            let node = pr.node;
                            let warn = pr.level == Level::Warning;
                            div()
                                .id(("problem", i))
                                .debug_selector(move || format!("problem-{i}"))
                                .flex()
                                .items_start()
                                .gap(px(9.))
                                .px(px(14.))
                                .py(px(5.))
                                .text_size(px(12.5))
                                .cursor_pointer()
                                .hover(|d| d.bg(p.panel2))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if let Some(id) = node {
                                        this.select_node(id, window, cx);
                                    }
                                }))
                                .child(icon(if warn { "icons/warning.svg" } else { "icons/jm/dot.svg" }, 14., if warn { p.warn } else { p.muted }))
                                .child(div().flex_1().child(pr.text.clone()))
                        }))
                        .into_any_element()
                }
            }
        };
        div()
            .id("bottom")
            .flex_none()
            .h(px(self.workspace.bottom.size))
            .w_full()
            .flex()
            .flex_col()
            .bg(p.panel)
            .border_t_1()
            .border_color(p.line)
            .child(
                div()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(p.line)
                    .child(tab("bottom-run", "Run".into(), BottomTab::Run, cx))
                    .child(tab("bottom-requests", format!("Requests{}", self.run.as_ref().map_or(String::new(), |r| if r.details.is_empty() { String::new() } else { format!(" ({})", r.details.len()) })), BottomTab::Requests, cx))
                    .child(tab("bottom-history", format!("History{}", if self.history_entries().is_empty() { String::new() } else { format!(" ({})", self.history_entries().len()) }), BottomTab::History, cx))
                    .child(tab("bottom-problems", format!("Problems{}", if problems.is_empty() { String::new() } else { format!(" ({})", problems.len()) }), BottomTab::Problems, cx))
                    .child(div().flex_1())
                    .child(
                        IconButton::new("bottom-close", IconName::Close)
                            .icon_size(IconSize::XSmall)
                            .tooltip(Tooltip::text("Hide panel (Ctrl+J)"))
                            .on_click(cx.listener(|this, _, window, cx| this.perform(Action::ToggleBottomPanel, window, cx))),
                    ),
            )
            .child(div().id("bottom-content").flex_1().min_h_0().when(!matches!(self.bottom_tab, BottomTab::Run | BottomTab::Requests), |d| d.overflow_y_scroll()).child(content))
    }

    // ---- status bar ----

    pub(crate) fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let warnings = self.plan().map(|lp| lp.plan_notes().iter().filter(|x| x.level == Level::Warning).count()).unwrap_or(0);
        let left = match (&self.notice, &self.run) {
            (Some(n), _) => format!("{} {}", if n.severity == Severity::Error { "\u{26a0}" } else { "\u{25cf}" }, n.text),
            (None, Some(r)) if r.phase.active() => format!("\u{25b6} {} {}: {} samples, {}", r.kind.label(), r.phase.label().to_lowercase(), crate::views_run::thousands(r.live.total().count), crate::views_run::clock(r.elapsed().as_secs())),
            (None, _) => "\u{25cf} Ready".to_string(),
        };
        let runtime = match (&self.runtime.java, &self.runtime.jmeter) {
            (Some(j), Some(m)) => format!("{} \u{b7} JMeter {m}", short_java(j)),
            (Some(j), None) => short_java(j),
            _ => "Java not found".to_string(),
        };
        div()
            .flex_none()
            .h(px(crate::views::STATUSBAR_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .gap(px(16.))
            .px(px(12.))
            .bg(p.accent)
            .text_color(p.on_accent)
            .text_size(px(11.5))
            .child(div().max_w(px(520.)).overflow_hidden().whitespace_nowrap().child(left))
            .when_some(self.plan(), |d, lp| d.child(div().child(format!("{} \u{b7} {} elements", lp.name(), lp.plan.len()))))
            .when(warnings > 0, |d| d.child(div().child(format!("{warnings} warning{}", if warnings == 1 { "" } else { "s" }))))
            .child(div().ml_auto().child(runtime))
    }

    // ---- dialogs ----

    pub(crate) fn about_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        div()
            .id("about-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(with_alpha(gpui::black(), 0.4))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.about_open = false;
                cx.notify();
            }))
            .child(
                div()
                    .id("about")
                    .w(px(420.))
                    .p(px(24.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .bg(p.raise)
                    .border_1()
                    .border_color(p.line)
                    .rounded(px(10.))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(div().flex().items_center().gap(px(10.)).child(icon("icons/jm/zap.svg", 28., p.accent)).child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child("JMeter UI")))
                    .child(div().text_color(p.muted).child(format!("Version {}", env!("CARGO_PKG_VERSION"))))
                    .child(div().child("A desktop front end for Apache JMeter. It edits test plans and runs JMeter in non-GUI mode."))
                    .child(div().text_size(px(12.)).text_color(p.muted).child("Licensed under the GNU General Public License, version 3 or later. Built with GPUI and components from Zed."))
                    .child(div().flex().justify_end().child(Button::new("about-close", "Close").style(ButtonStyle::Tinted(ui::TintColor::Accent)).on_click(cx.listener(|this, _, _, cx| {
                        this.about_open = false;
                        cx.notify();
                    })))),
            )
    }

    /// The question shown over the window: unsaved changes, reload, or recovered work.
    pub(crate) fn question_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let name_of = |id: shell_core::layout::PlanId| self.plans.iter().find(|lp| lp.id == id).map(|lp| lp.name()).unwrap_or_default();
        let (title, body, buttons): (String, String, Vec<(&'static str, &'static str, bool)>) = match &self.dialog {
            Some(Dialog::Unsaved { plans, then }) => {
                let names = plans.iter().map(|&id| name_of(id)).collect::<Vec<_>>().join(", ");
                let what = match then {
                    Then::Quit => "before closing the app",
                    _ => "before closing",
                };
                (
                    if plans.len() == 1 { "Save changes?".to_string() } else { format!("Save changes to {} plans?", plans.len()) },
                    format!("{names} {} unsaved changes. Save {what}?", if plans.len() == 1 { "has" } else { "have" }),
                    vec![("dialog-save", "Save", true), ("dialog-discard", "Don't Save", false), ("dialog-cancel", "Cancel", false)],
                )
            }
            Some(Dialog::NewPlan) => (
                "New plan".to_string(),
                format!(
                    "Choose a starting point. You pick where to save it next.\n\n{}",
                    plan_core::starters::Starter::ALL.iter().map(|s| format!("{}: {}.", s.label(), s.description())).collect::<Vec<_>>().join("\n")
                ),
                vec![("dialog-new-api", "API test", true), ("dialog-new-web", "Web site test", false), ("dialog-new-empty", "Empty plan", false), ("dialog-cancel", "Cancel", false)],
            ),
            Some(Dialog::Curl { .. }) => (
                "Import from cURL".to_string(),
                "Paste one or more curl commands, for example from \"Copy as cURL\" in a browser's developer tools. Each becomes an HTTP request; its headers go in an HTTP Header Manager under it.".to_string(),
                vec![("dialog-curl-add", "Add to Plan", true), ("dialog-cancel", "Cancel", false)],
            ),
            Some(Dialog::DiscardForReload(id)) => (
                "Discard changes?".to_string(),
                format!("Reloading {} replaces your unsaved changes with the file on disk.", name_of(*id)),
                vec![("dialog-reload", "Discard and Reload", true), ("dialog-cancel", "Cancel", false)],
            ),
            Some(Dialog::Recover { path, saved, .. }) => (
                "Recover unsaved changes?".to_string(),
                format!(
                    "{} has unsaved changes from an earlier session that did not close properly{}.",
                    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                    if saved.is_some() { "" } else { ". The plan file itself is gone" }
                ),
                vec![("dialog-recover", "Recover Changes", true), ("dialog-discard-recovery", "Discard Them", false)],
            ),
            Some(Dialog::SaveToRun { kind, .. }) => (
                "Save before running?".to_string(),
                format!("A run uses the saved file, and this plan has unsaved changes. Save it and {}?", match kind {
                    jvm_runtime::runs::RunKind::Run => "run it",
                    jvm_runtime::runs::RunKind::Validate => "validate it",
                    jvm_runtime::runs::RunKind::SendOnce => "send the request",
                }),
                vec![("dialog-save-run", "Save and Run", true), ("dialog-cancel", "Cancel", false)],
            ),
            Some(Dialog::Scripts { text, .. }) => (
                "This plan can run code".to_string(),
                format!("{text} Run it only if you trust where this plan came from. You are asked once for each plan file."),
                vec![("dialog-run-scripts", "Run Anyway", true), ("dialog-cancel", "Cancel", false)],
            ),
            Some(Dialog::RunsPlace { message, .. }) => (
                "Where should run files go?".to_string(),
                format!("{message}. Run files are kept in a runs folder, next to the .jmx file or in a folder you choose."),
                vec![("dialog-use-plan-folder", "Use the .jmx Folder", true), ("dialog-choose-folder", "Choose Folder\u{2026}", false), ("dialog-cancel", "Cancel", false)],
            ),
            Some(Dialog::ClearRuns { root, count }) => (
                "Clear the runs folder?".to_string(),
                format!("This removes {count} run{} (results, logs and reports) from {}. Other files in it are left alone.", if *count == 1 { "" } else { "s" }, root.display()),
                vec![("dialog-clear-runs", "Clear Runs", true), ("dialog-cancel", "Cancel", false)],
            ),
            Some(Dialog::DeleteRun { name, .. }) => (
                "Remove this run?".to_string(),
                format!("This removes {name} with its results, log and reports."),
                vec![("dialog-delete-run", "Remove Run", true), ("dialog-cancel", "Cancel", false)],
            ),
            Some(Dialog::QuitRunning) => (
                "A run is going".to_string(),
                "Quitting stops the run. Its results so far are kept in the run folder.".to_string(),
                vec![("dialog-stop-quit", "Stop and Quit", true), ("dialog-cancel", "Cancel", false)],
            ),
            None => (String::new(), String::new(), Vec::new()),
        };
        let curl = match &self.dialog {
            Some(Dialog::Curl { area, error }) => Some((area.clone(), error.clone())),
            _ => None,
        };
        let buttons = buttons.into_iter().map(|(id, label, primary)| {
            let b = Button::new(id, label).style(if primary { ButtonStyle::Tinted(ui::TintColor::Accent) } else { ButtonStyle::Subtle });
            div().debug_selector(move || id.to_string()).child(b.on_click(cx.listener(move |this, _, window, cx| {
            match id {
                "dialog-save" => this.dialog_save(window, cx),
                "dialog-discard" => this.dialog_discard(window, cx),
                "dialog-reload" => this.dialog_reload(window, cx),
                "dialog-curl-add" => this.dialog_curl_add(window, cx),
                "dialog-new-api" => this.dialog_new_plan(plan_core::starters::Starter::Api, window, cx),
                "dialog-new-web" => this.dialog_new_plan(plan_core::starters::Starter::Web, window, cx),
                "dialog-new-empty" => this.dialog_new_plan(plan_core::starters::Starter::Empty, window, cx),
                "dialog-recover" => this.resolve_recovery(true, window, cx),
                "dialog-discard-recovery" => this.resolve_recovery(false, window, cx),
                "dialog-save-run" => this.dialog_save_and_run(window, cx),
                "dialog-run-scripts" => this.dialog_run_scripts(window, cx),
                "dialog-use-plan-folder" => this.dialog_use_plan_folder(window, cx),
                "dialog-choose-folder" => this.dialog_choose_runs_folder(window, cx),
                "dialog-clear-runs" => this.dialog_clear_runs(window, cx),
                "dialog-delete-run" => this.dialog_delete_run(window, cx),
                "dialog-stop-quit" => this.dialog_stop_and_quit(window, cx),
                _ => this.dialog_cancel(cx),
            }
            // A question that held the keyboard focus (the cURL dialog has a text area) gives it back to the window,
            // so the shortcuts work again without a click.
            if this.dialog.is_none() {
                window.focus(&this.focus, cx);
            }
            })))
        });
        div()
            .id("dialog-overlay")
            .debug_selector(|| "dialog".to_string())
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(with_alpha(gpui::black(), 0.4))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w(px(if matches!(self.dialog, Some(Dialog::Curl { .. })) { 680. } else { 440. }))
                    .max_w_full()
                    .p(px(22.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .bg(p.raise)
                    .border_1()
                    .border_color(p.line)
                    .rounded(px(10.))
                    .shadow_lg()
                    .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(title))
                    .child(div().text_color(p.muted).child(body))
                    .when_some(curl, |d, (area, error)| {
                        d.child(div().debug_selector(|| "curl-text".to_string()).child(area))
                            .when_some(error, |d, e| d.child(div().debug_selector(|| "curl-error".to_string()).text_color(p.err).text_size(px(12.5)).child(e)))
                    })
                    .child(div().flex().justify_end().gap(px(8.)).children(buttons)),
            )
    }

    pub(crate) fn toast(&self, n: Notice, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let color = if n.severity == Severity::Error { p.err } else { p.ok };
        div()
            .id("toast")
            .absolute()
            .right(px(16.))
            .bottom(px(crate::views::STATUSBAR_HEIGHT + 14.))
            .max_w(px(460.))
            .flex()
            .items_start()
            .gap(px(10.))
            .px(px(12.))
            .py(px(9.))
            .bg(p.raise)
            .border_1()
            .border_color(with_alpha(color, 0.7))
            .rounded(px(8.))
            .shadow_lg()
            .text_size(px(12.5))
            .child(icon(if n.severity == Severity::Error { "icons/warning.svg" } else { "icons/check.svg" }, 15., color))
            .child(div().flex_1().child(n.text))
            .child(IconButton::new("toast-close", IconName::Close).icon_size(IconSize::XSmall).on_click(cx.listener(|this, _, _, cx| this.dismiss_notice(cx))))
    }
}

/// `openjdk version "21.0.8" 2025-07-15 LTS` becomes `Java 21.0.8`.
pub fn short_java(line: &str) -> String {
    match line.split('"').nth(1) {
        Some(v) => format!("Java {v}"),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_version_is_shortened() {
        assert_eq!(short_java(r#"openjdk version "21.0.8" 2025-07-15 LTS"#), "Java 21.0.8");
        assert_eq!(short_java(r#"java version "1.8.0_391""#), "Java 1.8.0_391");
        assert_eq!(short_java("weird"), "weird");
    }
}
