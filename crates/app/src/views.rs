//! Drawing the window: title bar, tree, tab bar and editor, inspector, results panel and status bar.

use crate::assets::{MONO_FONT, UI_FONT};
use crate::shell::{Resize, Shell, TreeDown, TreeFirst, TreeLast, TreeLeft, TreeOpen, TreePageDown, TreePageUp, TreeRight, TreeToggle, TreeUp};
use crate::theme::{self, with_alpha};
use crate::ui_util::icon;
use gpui::{
    AnyElement, Anchor, Context, FontWeight, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, WindowControlArea, Window,
    div, prelude::*, px, uniform_list,
};
use plan_core::{Category, Move};
use crate::shell::{DropZone, TreeDrag};
use shell_core::actions::{Action, Availability};
use shell_core::layout::{Tab, TabKind};
use std::ops::Range;
use std::rc::Rc;
use ui::{Button, ButtonCommon, ButtonSize, ButtonStyle, Clickable, ContextMenu, ContextMenuEntry, ContextMenuItem, Disableable, IconButton, IconName, IconSize, PopoverMenu, Tooltip, right_click_menu};

pub const ACTIVITY_WIDTH: f32 = 44.0;
pub const TITLEBAR_HEIGHT: f32 = 38.0;
pub const STATUSBAR_HEIGHT: f32 = 24.0;
const HANDLE: f32 = 5.0;

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_form(window, cx);
        self.give_back_viewer_focus(window, cx);
        self.write_state_dump(cx);
        let p = theme::palette(cx);
        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .child(self.activity_bar(cx))
            .when(self.workspace.sidebar.visible, |d| d.child(self.sidebar(window, cx)).child(self.handle(Resize::Sidebar, true, cx)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(self.editor_area(window, cx))
                    .when(self.workspace.bottom.visible, |d| d.child(self.handle(Resize::Bottom, false, cx)).child(self.bottom_panel(cx))),
            )
            .when(self.workspace.inspector.visible, |d| d.child(self.handle(Resize::Inspector, true, cx)).child(self.inspector(cx)));

        let root = div()
            .id("shell")
            .key_context("Shell")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.fg)
            .font_family(UI_FONT)
            .text_size(px(13.))
            .relative()
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up));
        let root = self.attach_actions(root, cx);
        root.child(self.titlebar(window, cx))
            .child(body)
            .child(self.status_bar(cx))
            .children(self.palette.clone().map(|pal| pal.into_any_element()))
            .when(self.about_open, |d| d.child(self.about_dialog(cx)))
            .when(self.dialog.is_some(), |d| d.child(self.question_dialog(cx)))
            .when_some(self.notice.clone(), |d, n| d.child(self.toast(n, cx)))
    }
}

// ---- resizing ----

impl Shell {
    fn handle(&self, kind: Resize, vertical: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let id = match kind {
            Resize::Sidebar => "resize-sidebar",
            Resize::Inspector => "resize-inspector",
            Resize::Bottom => "resize-bottom",
        };
        let active = self.resizing == Some(kind);
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .flex_none()
            .when(vertical, |d| d.w(px(HANDLE)).h_full().cursor_col_resize())
            .when(!vertical, |d| d.h(px(HANDLE)).w_full().cursor_row_resize())
            .bg(if active { p.accent } else { with_alpha(p.line, 0.0) })
            .hover(|d| d.bg(with_alpha(p.accent, 0.6)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                this.resizing = Some(kind);
                cx.notify();
            }))
    }

    pub fn on_mouse_move(&mut self, e: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.resizing else { return };
        if !e.dragging() {
            self.resizing = None;
            return;
        }
        let size = window.viewport_size();
        let (x, y) = (f32::from(e.position.x), f32::from(e.position.y));
        let (w, h) = (f32::from(size.width), f32::from(size.height));
        match kind {
            Resize::Sidebar => self.workspace.sidebar.resize(x - ACTIVITY_WIDTH),
            Resize::Inspector => self.workspace.inspector.resize(w - x),
            Resize::Bottom => self.workspace.bottom.resize(h - y - STATUSBAR_HEIGHT),
        }
        cx.notify();
    }

    pub fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.resizing.take().is_some() {
            self.save_settings();
            cx.notify();
        }
    }
}

// ---- title bar and menus ----

enum MenuItem {
    Action(Action),
    Separator,
    Recent,
}

impl Shell {
    fn titlebar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let menus: [(&'static str, Vec<MenuItem>); 6] = [
            ("File", vec![
                MenuItem::Action(Action::NewPlan),
                MenuItem::Action(Action::OpenPlan),
                MenuItem::Recent,
                MenuItem::Separator,
                MenuItem::Action(Action::SavePlan),
                MenuItem::Action(Action::SavePlanAs),
                MenuItem::Action(Action::ReloadPlan),
                MenuItem::Separator,
                MenuItem::Action(Action::MergePlan),
                MenuItem::Action(Action::ImportCurl),
                MenuItem::Action(Action::SaveSelectionAs),
                MenuItem::Separator,
                MenuItem::Action(Action::CloseTab),
                MenuItem::Action(Action::CloseAllTabs),
                MenuItem::Separator,
                MenuItem::Action(Action::OpenSettings),
                MenuItem::Action(Action::Quit),
            ]),
            ("Edit", vec![
                MenuItem::Action(Action::Undo),
                MenuItem::Action(Action::Redo),
                MenuItem::Separator,
                MenuItem::Action(Action::CutElement),
                MenuItem::Action(Action::CopyElement),
                MenuItem::Action(Action::PasteElement),
                MenuItem::Separator,
                MenuItem::Action(Action::AddElement),
                MenuItem::Action(Action::DuplicateElement),
                MenuItem::Action(Action::DeleteElement),
                MenuItem::Action(Action::RenameElement),
                MenuItem::Action(Action::ToggleElementEnabled),
                MenuItem::Separator,
                MenuItem::Action(Action::MoveElementUp),
                MenuItem::Action(Action::MoveElementDown),
                MenuItem::Separator,
                MenuItem::Action(Action::GotoElement),
            ]),
            (
                "Run",
                vec![
                    MenuItem::Action(Action::RunPlan),
                    MenuItem::Action(Action::ValidatePlan),
                    MenuItem::Action(Action::SendOnce),
                    MenuItem::Action(Action::StopRun),
                    MenuItem::Action(Action::StopRunNow),
                    MenuItem::Action(Action::ViewRequests),
                    MenuItem::Action(Action::OpenRunsFolder),
                    MenuItem::Action(Action::ClearRunsFolder),
                ],
            ),
            ("View", vec![
                MenuItem::Action(Action::ToggleSidebar),
                MenuItem::Action(Action::ToggleInspector),
                MenuItem::Action(Action::ToggleBottomPanel),
                MenuItem::Separator,
                MenuItem::Action(Action::ExpandAll),
                MenuItem::Action(Action::CollapseAll),
                MenuItem::Separator,
                MenuItem::Action(Action::ThemeSystem),
                MenuItem::Action(Action::ThemeDark),
                MenuItem::Action(Action::ThemeLight),
            ]),
            ("Tools", vec![MenuItem::Action(Action::CommandPalette), MenuItem::Action(Action::GotoElement), MenuItem::Action(Action::FocusTree)]),
            ("Help", vec![MenuItem::Action(Action::About)]),
        ];
        let shell = cx.entity();
        let menu_buttons = menus.into_iter().map(|(label, items)| {
            let shell = shell.clone();
            let items = Rc::new(items);
            PopoverMenu::new(label)
                .anchor(Anchor::TopLeft)
                .menu(move |window, cx| {
                    let shell = shell.clone();
                    let items = items.clone();
                    Some(ContextMenu::build(window, cx, move |mut menu, _, cx| {
                        let ctx = shell.read(cx).context();
                        let recents = shell.read(cx).settings.recent_files.clone();
                        for item in items.iter() {
                            match item {
                                MenuItem::Separator => menu = menu.separator(),
                                MenuItem::Recent => {
                                    if !recents.is_empty() {
                                        menu = menu.separator().header("Recent Plans");
                                        for path in recents.iter().take(6) {
                                            let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
                                            let (shell, path) = (shell.clone(), path.clone());
                                            menu = menu.entry(name, None, move |window, cx| {
                                                let path = path.clone();
                                                shell.update(cx, |s, cx| {
                                                    s.open_plan_path(std::path::Path::new(&path), window, cx);
                                                });
                                            });
                                        }
                                        menu = menu.separator();
                                    }
                                }
                                MenuItem::Action(a) => {
                                    let a = *a;
                                    let shell = shell.clone();
                                    let entry = ContextMenuEntry::new(a.title())
                                        .action(crate::shell::boxed(a))
                                        .disabled(!matches!(a.availability(&ctx), Availability::Enabled))
                                        .handler(move |window, cx| {
                                            shell.update(cx, |s, cx| s.perform(a, window, cx));
                                        });
                                    menu = menu.item(ContextMenuItem::Entry(entry));
                                }
                            }
                        }
                        menu
                    }))
                })
                .trigger(Button::new(gpui::SharedString::from(format!("menu-trigger-{label}")), label).style(ButtonStyle::Subtle).size(ButtonSize::Compact))
        });

        let controls = |label: &'static str, area: WindowControlArea, hover: gpui::Hsla, id: &'static str| {
            div()
                .id(id)
                .w(px(46.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(p.muted)
                .window_control_area(area)
                .hover(move |d| d.bg(hover).text_color(p.fg))
                .child(label)
        };

        div()
            .h(px(TITLEBAR_HEIGHT))
            .flex_none()
            .w_full()
            .flex()
            .items_center()
            .bg(p.panel)
            .border_b_1()
            .border_color(p.line)
            .child(div().flex().items_center().gap(px(7.)).pl(px(12.)).pr(px(10.)).child(icon("icons/jm/zap.svg", 16., p.accent)).child(div().font_weight(FontWeight::SEMIBOLD).child("JMeter")))
            .child(div().flex().items_center().children(menu_buttons))
            .child(div().flex_1().h_full().window_control_area(WindowControlArea::Drag))
            .child(
                div()
                    .id("palette-trigger")
                    .debug_selector(|| "palette-trigger".to_string())
                    .w(px(380.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(3.))
                    .rounded(px(6.))
                    .bg(p.bg)
                    .border_1()
                    .border_color(p.line)
                    .text_color(p.muted)
                    .text_size(px(12.5))
                    .cursor_pointer()
                    .hover(|d| d.border_color(p.accent))
                    .on_click(cx.listener(|this, _, window, cx| this.perform(Action::CommandPalette, window, cx)))
                    .child(icon("icons/magnifying_glass.svg", 14., p.muted))
                    .child("Search commands and elements")
                    .child(div().ml_auto().px(px(5.)).rounded(px(3.)).border_1().border_color(p.line).text_size(px(10.5)).font_family(MONO_FONT).child("Ctrl K")),
            )
            .child(div().flex_1().h_full().window_control_area(WindowControlArea::Drag))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .pr(px(10.))
                    .child(div().debug_selector(|| "run".to_string()).child(
                        Button::new("run", "Run")
                            .style(ButtonStyle::Tinted(ui::TintColor::Accent))
                            .size(ButtonSize::Compact)
                            .disabled(!Action::RunPlan.availability(&self.context()).is_enabled())
                            .tooltip(Tooltip::text("Run the saved plan (F5)"))
                            .on_click(cx.listener(|this, _, window, cx| this.perform(Action::RunPlan, window, cx))),
                    ))
                    .child(div().debug_selector(|| "stop".to_string()).child(
                        Button::new("stop", "Stop")
                            .style(ButtonStyle::Subtle)
                            .size(ButtonSize::Compact)
                            .disabled(!Action::StopRun.availability(&self.context()).is_enabled())
                            .tooltip(Tooltip::text("Stop the run after the current iteration (Shift+F5)"))
                            .on_click(cx.listener(|this, _, window, cx| this.perform(Action::StopRun, window, cx))),
                    )),
            )
            .child(controls("\u{2013}", WindowControlArea::Min, p.panel2, "win-min"))
            .child(controls("\u{25a1}", WindowControlArea::Max, p.panel2, "win-max"))
            .child(controls("\u{00d7}", WindowControlArea::Close, with_alpha(p.err, 0.8), "win-close"))
    }

    fn activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let item = |id: &'static str, icon_path: &'static str, tip: &'static str, on: bool, action: Action, cx: &mut Context<Self>| {
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .relative()
                .w(px(ACTIVITY_WIDTH))
                .h(px(38.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(if on { p.fg } else { p.muted })
                .hover(|d| d.text_color(p.fg))
                .tooltip(Tooltip::text(tip))
                .when(on, |d| d.child(div().absolute().left_0().top(px(7.)).bottom(px(7.)).w(px(2.)).bg(p.accent)))
                .on_click(cx.listener(move |this, _, window, cx| this.perform(action, window, cx)))
                .child(icon(icon_path, 20., if on { p.fg } else { p.muted }))
        };
        div()
            .flex_none()
            .w(px(ACTIVITY_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .pt(px(8.))
            .bg(p.panel)
            .border_r_1()
            .border_color(p.line)
            .child(item("act-plan", "icons/jm/flask.svg", "Test Plan (Ctrl+B)", self.workspace.sidebar.visible, Action::ToggleSidebar, cx))
            .child(item("act-results", "icons/jm/chart.svg", "Results Panel (Ctrl+J)", self.workspace.bottom.visible, Action::ToggleBottomPanel, cx))
            .child(div().flex_1())
            .child(item("act-settings", "icons/settings.svg", "Settings (Ctrl+,)", self.workspace.active_tab().is_some_and(|t| t.kind == TabKind::Settings), Action::OpenSettings, cx))
            .child(div().h(px(8.)))
    }
}

// ---- sidebar: the plan tree ----

impl Shell {
    fn sidebar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let has_plan = self.plan().is_some();
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.))
            .pt(px(8.))
            .pb(px(6.))
            .child(div().text_size(px(11.)).font_family(MONO_FONT).text_color(p.muted).child("TEST PLAN"))
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .child(
                        IconButton::new("expand-all", IconName::ChevronDown)
                            .icon_size(IconSize::XSmall)
                            .disabled(!has_plan)
                            .tooltip(Tooltip::text("Expand all"))
                            .on_click(cx.listener(|this, _, window, cx| this.perform(Action::ExpandAll, window, cx))),
                    )
                    .child(
                        IconButton::new("collapse-all", IconName::ListCollapse)
                            .icon_size(IconSize::XSmall)
                            .disabled(!has_plan)
                            .tooltip(Tooltip::text("Collapse all"))
                            .on_click(cx.listener(|this, _, window, cx| this.perform(Action::CollapseAll, window, cx))),
                    ),
            );
        let body: AnyElement = match self.plan() {
            Some(lp) => {
                let rows = Rc::new(lp.tree.rows(&lp.plan));
                let count = rows.len();
                div()
                    .id("tree")
                    .key_context("Tree")
                    .track_focus(&self.tree_focus)
                    .flex_1()
                    .min_h_0()
                    .on_action(cx.listener(|this, _: &TreeUp, window, cx| this.tree_move(Move::Up, window, cx)))
                    .on_action(cx.listener(|this, _: &TreeDown, window, cx| this.tree_move(Move::Down, window, cx)))
                    .on_action(cx.listener(|this, _: &TreeFirst, window, cx| this.tree_move(Move::First, window, cx)))
                    .on_action(cx.listener(|this, _: &TreeLast, window, cx| this.tree_move(Move::Last, window, cx)))
                    .on_action(cx.listener(|this, _: &TreePageUp, window, cx| this.tree_move(Move::PageUp(10), window, cx)))
                    .on_action(cx.listener(|this, _: &TreePageDown, window, cx| this.tree_move(Move::PageDown(10), window, cx)))
                    .on_action(cx.listener(|this, _: &TreeLeft, _, cx| this.tree_key_left(cx)))
                    .on_action(cx.listener(|this, _: &TreeRight, _, cx| this.tree_key_right(cx)))
                    .on_action(cx.listener(|this, _: &TreeOpen, _, cx| this.toggle_selected(cx)))
                    .on_action(cx.listener(|this, _: &TreeToggle, _, cx| this.toggle_selected(cx)))
                    .child(
                        uniform_list("tree-rows", count, cx.processor(move |this: &mut Shell, range: Range<usize>, _w, cx| {
                            range.map(|i| this.tree_row(&rows[i], i, cx)).collect::<Vec<_>>()
                        }))
                        .track_scroll(&self.tree_scroll)
                        .size_full(),
                    )
                    .into_any_element()
            }
            None => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .px(px(16.))
                .text_color(p.muted)
                .text_size(px(12.5))
                .child("No plan open")
                .child(Button::new("sidebar-new", "New Plan\u{2026}").style(ButtonStyle::Subtle).on_click(cx.listener(|this, _, window, cx| this.perform(Action::NewPlan, window, cx))))
                .child(Button::new("sidebar-open", "Open Plan\u{2026}").style(ButtonStyle::Tinted(ui::TintColor::Accent)).on_click(cx.listener(|this, _, window, cx| this.perform(Action::OpenPlan, window, cx))))
                .into_any_element(),
        };
        div().flex_none().w(px(self.workspace.sidebar.size)).h_full().flex().flex_col().bg(p.panel).child(header).child(body)
    }

    fn tree_row(&self, row: &plan_core::Row, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let Some(lp) = self.plan() else { return div().id(("tree-row", index)).into_any_element() };
        let Some(node) = lp.plan.node(row.id) else { return div().id(("tree-row", index)).into_any_element() };
        let plan_id = lp.id;
        let label = node.name.clone();
        let (category_icon, category_color) = (node.category.icon(), p.category(node.category));
        let selected = lp.tree.selected() == Some(row.id);
        let id = row.id;
        let color = p.category(node.category);
        let name_color = if node.enabled { p.fg } else { with_alpha(p.fg, 0.45) };
        let zone = |zone: DropZone, cx: &mut Context<Self>| {
            let (id_part, top, height) = match zone {
                DropZone::Before => ("before", Some(0.), 6.),
                DropZone::After => ("after", None, 6.),
                DropZone::Into => ("into", Some(6.), 13.),
            };
            let z = div()
                .id((gpui::SharedString::from(format!("drop-{id_part}")), index))
                .debug_selector(move || format!("drop-{id_part}-{index}"))
                .absolute()
                .left_0()
                .right_0()
                .h(px(height))
                .on_drop(cx.listener(move |this, drag: &TreeDrag, window, cx| this.drop_node(drag, id, zone, window, cx)))
                .drag_over::<TreeDrag>(move |s, _, _, _| match zone {
                    DropZone::Before => s.border_t_2().border_color(p.accent),
                    DropZone::After => s.border_b_2().border_color(p.accent),
                    DropZone::Into => s.bg(with_alpha(p.accent, 0.28)),
                });
            match top {
                Some(t) => z.top(px(t)),
                None => z.bottom_0(),
            }
        };
        let can_drag = row.depth > 0;
        let row_el = div()
            .id(("tree-row", index))
            .debug_selector(move || format!("tree-row-{index}"))
            .relative()
            .h(px(25.))
            .w_full()
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(8. + 14. * row.depth as f32))
            .pr(px(10.))
            .cursor_pointer()
            .border_l_2()
            .border_color(if selected { p.accent } else { with_alpha(p.accent, 0.0) })
            .when(selected, |d| d.bg(p.selection()))
            .when(!selected, |d| d.hover(|d| d.bg(with_alpha(p.panel2, 0.8))))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                this.select_node(id, window, cx);
                window.focus(&this.tree_focus, cx);
                if e.click_count == 2 {
                    this.toggle_node(id, cx);
                }
            }))
            .child(
                div()
                    .id(("chev", index))
                    .debug_selector(move || format!("tree-chev-{index}"))
                    .w(px(14.))
                    .h(px(14.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(row.has_children, |d| {
                        d.child(icon(if row.expanded { "icons/chevron_down.svg" } else { "icons/chevron_right.svg" }, 12., p.muted))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.toggle_node(id, cx);
                            }))
                    }),
            )
            .child(icon(node.category.icon(), 15., if node.enabled { color } else { with_alpha(color, 0.45) }))
            .child(div().flex_1().min_w_0().overflow_hidden().whitespace_nowrap().text_color(name_color).when(!node.enabled, |d| d.line_through()).child(node.name.clone()))
            .when(node.name != node.type_label && row.depth > 0 && !matches!(node.category, Category::Sampler), |d| {
                d.child(div().flex_none().text_size(px(10.5)).font_family(MONO_FONT).text_color(p.muted).child(short_type(&node.type_label)))
            })
            .when(can_drag, |d| {
                d.on_drag(TreeDrag { plan: plan_id, node: id, label: label.clone(), icon: category_icon, color: category_color }, |drag, _, _, cx| cx.new(|_| DragPreview(drag.clone())))
            })
            .child(zone(DropZone::Before, cx))
            .child(zone(DropZone::Into, cx))
            .child(zone(DropZone::After, cx));
        let shell = cx.entity();
        right_click_menu::<ContextMenu>(("row-menu", index))
            .trigger(move |_, _, _| row_el)
            .menu(move |window, cx| {
                let shell = shell.clone();
                shell.update(cx, |s, cx| s.select_node(id, window, cx));
                Tree::build_menu(shell, id, window, cx)
            })
            .into_any_element()
    }
}

/// A short form of a type name for the right side of a tree row.
fn short_type(label: &str) -> String {
    label.replace(" Controller", "").replace(" Config", "")
}

// ---- editor area: one tab per plan ----

impl Shell {
    fn editor_area(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let active = self.workspace.active();
        let tabs = self.workspace.tabs().iter().enumerate().map(|(i, t)| self.tab_view(i, t, active == Some(i), cx));
        let tab_bar = div().flex_none().h(px(34.)).flex().items_center().bg(p.panel).border_b_1().border_color(p.line).child(div().flex_1().min_w_0().flex().overflow_hidden().children(tabs));
        let content: AnyElement = match self.workspace.active_tab() {
            Some(t) => self.tab_content(t, window, cx),
            None => self.welcome(cx),
        };
        div()
            .id("editor")
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .child(tab_bar)
            .child(div().id("editor-content").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.editor_scroll).child(content))
    }

    fn tab_view(&self, index: usize, tab: &Tab, active: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let (icon_path, color) = match tab.kind {
            TabKind::Plan(_) => ("icons/jm/flask.svg", p.category(Category::TestPlan)),
            TabKind::Settings => ("icons/settings.svg", p.muted),
        };
        let title = tab.title.clone();
        let dirty = tab.dirty;
        div()
            .id(("tab", index))
            .debug_selector(move || format!("tab-{index}"))
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .gap(px(7.))
            .pl(px(12.))
            .pr(px(6.))
            .border_r_1()
            .border_color(p.line)
            .cursor_pointer()
            .text_size(px(12.5))
            .when(active, |d| d.bg(p.bg).text_color(p.fg).border_t_2().border_color(p.accent))
            .when(!active, |d| d.text_color(p.muted).hover(|d| d.bg(p.panel2).text_color(p.fg)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| this.activate_tab(index, window, cx)))
            .on_mouse_down(MouseButton::Middle, cx.listener(move |this, _: &MouseDownEvent, window, cx| this.request_close_tab(index, window, cx)))
            .child(icon(icon_path, 14., color))
            .child(div().whitespace_nowrap().child(title))
            .when(dirty, |d| d.child(div().w(px(7.)).h(px(7.)).rounded_full().bg(p.accent)))
            .child(IconButton::new(("tab-close", index), IconName::Close).icon_size(IconSize::XSmall).tooltip(Tooltip::text("Close")).on_click(cx.listener(move |this, _, window, cx| this.request_close_tab(index, window, cx))))
            .into_any_element()
    }
}

// ---- dragging and the right-click menu in the tree ----

/// What follows the pointer while an element is dragged.
struct DragPreview(TreeDrag);

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(p.raise)
            .border_1()
            .border_color(p.accent)
            .text_size(px(12.5))
            .text_color(p.fg)
            .shadow_lg()
            .child(icon(self.0.icon, 14., self.0.color))
            .child(self.0.label.clone())
    }
}

struct Tree;

impl Tree {
    /// The menu for a tree element: add something under it, copy it, delete it, and so on.
    fn build_menu(shell: gpui::Entity<Shell>, id: plan_core::NodeId, window: &mut Window, cx: &mut gpui::App) -> gpui::Entity<ContextMenu> {
        ContextMenu::build(window, cx, move |mut menu, _, cx| {
            let ctx = shell.read(cx).context();
            let category = shell.read(cx).plan().and_then(|lp| lp.plan.node(id)).map(|n| n.category);
            let enabled = shell.read(cx).plan().and_then(|lp| lp.plan.node(id)).map(|n| n.enabled).unwrap_or(true);
            if let Some(category) = category {
                let allowed = plan_core::rules::allowed_children(category);
                if !allowed.is_empty() {
                    let sh = shell.clone();
                    menu = menu.submenu("Add", move |mut sub, _, _| {
                        for c in &allowed {
                            let sh = sh.clone();
                            let types: Vec<_> = plan_core::catalog::addable().into_iter().filter(|t| t.category == *c).collect();
                            sub = sub.submenu(c.plural(), move |mut list, _, _| {
                                for t in &types {
                                    let (sh, tag, gui) = (sh.clone(), t.tag, t.gui.unwrap_or_default());
                                    list = list.entry(t.label, None, move |window, cx| {
                                        sh.update(cx, |s, cx| s.add_element(tag, gui, window, cx));
                                    });
                                }
                                list
                            });
                        }
                        sub
                    });
                    menu = menu.separator();
                }
            }
            let item = |menu: ContextMenu, label: String, action: Action, shell: &gpui::Entity<Shell>, ctx: &shell_core::actions::AppContext| {
                let sh = shell.clone();
                let entry = ContextMenuEntry::new(label)
                    .action(crate::shell::boxed(action))
                    .disabled(!matches!(action.availability(ctx), Availability::Enabled))
                    .handler(move |window, cx| {
                        sh.update(cx, |s, cx| s.perform(action, window, cx));
                    });
                menu.item(ContextMenuItem::Entry(entry))
            };
            menu = item(menu, "Cut".into(), Action::CutElement, &shell, &ctx);
            menu = item(menu, "Copy".into(), Action::CopyElement, &shell, &ctx);
            menu = item(menu, "Paste".into(), Action::PasteElement, &shell, &ctx);
            menu = menu.separator();
            menu = item(menu, "Duplicate".into(), Action::DuplicateElement, &shell, &ctx);
            menu = item(menu, "Delete".into(), Action::DeleteElement, &shell, &ctx);
            menu = item(menu, "Rename".into(), Action::RenameElement, &shell, &ctx);
            menu = item(menu, if enabled { "Disable" } else { "Enable" }.into(), Action::ToggleElementEnabled, &shell, &ctx);
            menu = menu.separator();
            if category == Some(Category::Sampler) {
                menu = menu.separator();
                let sh = shell.clone();
                let ready = Action::SendOnce.availability(&crate::shell::AppContext { can_send_once: true, ..ctx }).is_enabled();
                let entry = ContextMenuEntry::new("Send once")
                    .disabled(!ready)
                    .handler(move |window, cx| {
                        sh.update(cx, |s, cx| {
                            s.select_node(id, window, cx);
                            s.perform(Action::SendOnce, window, cx);
                        });
                    });
                menu = menu.item(ContextMenuItem::Entry(entry));
            }
            menu = item(menu, "Move Up".into(), Action::MoveElementUp, &shell, &ctx);
            menu = item(menu, "Move Down".into(), Action::MoveElementDown, &shell, &ctx);
            menu = menu.separator();
            menu = item(menu, "Expand All".into(), Action::ExpandAll, &shell, &ctx);
            menu = item(menu, "Collapse All".into(), Action::CollapseAll, &shell, &ctx);
            menu
        })
    }
}
