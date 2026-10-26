//! Drawing the editor: the selected element's name, comments and settings.

use crate::assets::MONO_FONT;
use crate::form::{Cell, Item, SectionForm, TableForm, Widget};
use crate::shell::{LoadedPlan, Shell};
use crate::theme;
use crate::ui_util::{chip, icon};
use gpui::{AnyElement, Context, FontWeight, IntoElement, SharedString, Window, div, prelude::*, px};
use plan_core::forms::Kind;
use plan_core::NodeId;
use shell_core::layout::PlanId;
use ui::{Button, ButtonCommon, ButtonSize, ButtonStyle, Clickable, IconButton, IconName, IconSize, Tooltip};

const LABEL_WIDTH: f32 = 250.0;

impl Shell {
    /// What the middle area shows for a plan: whatever is selected in its tree.
    pub(crate) fn plan_editor(&self, plan_id: PlanId, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let Some(lp) = self.plans.iter().find(|lp| lp.id == plan_id) else { return div().into_any_element() };
        let root = lp.plan.roots().first().copied();
        let Some(id) = lp.tree.selected().or(root) else {
            return div().p(px(20.)).text_color(p.muted).child("This plan is empty.").into_any_element();
        };
        let Some(n) = lp.plan.node(id) else {
            return div().p(px(20.)).text_color(p.muted).child("This element is no longer in the plan.").into_any_element();
        };
        let color = p.category(n.category);
        let crumbs = lp.plan.breadcrumb(id);
        let last = crumbs.len().saturating_sub(1);
        let is_root = root == Some(id);
        let form = self.form.as_ref().filter(|f| f.plan == plan_id && f.node == id);
        div()
            .id("element")
            .debug_selector(|| "element-view".to_string())
            .flex()
            .flex_col()
            .child(
                div().flex().items_center().gap(px(6.)).px(px(20.)).py(px(7.)).border_b_1().border_color(p.line).text_size(px(12.)).text_color(p.muted).flex_wrap().children(crumbs.into_iter().enumerate().flat_map(|(i, c)| {
                    let mut v: Vec<AnyElement> = Vec::new();
                    if i > 0 {
                        v.push(icon("icons/chevron_right.svg", 11., p.muted).into_any_element());
                    }
                    v.push(div().when(i == last, |d| d.text_color(p.fg)).child(c).into_any_element());
                    v
                })),
            )
            .child(
                div()
                    .p(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(10.))
                            .child(icon(n.category.icon(), 22., color))
                            .child(div().debug_selector(|| "element-title".to_string()).flex_1().min_w(px(150.)).text_size(px(19.)).font_weight(FontWeight::SEMIBOLD).child(n.name.clone()))
                            .child(chip(n.type_label.clone(), color))
                            .child(self.enabled_button(n.enabled, cx)),
                    )
                    .when(is_root, |d| d.child(self.plan_summary(lp, cx)))
                    .when_some(form, |d, f| {
                        d.child(self.labelled("Name", div().debug_selector(|| "field-name".to_string()).child(f.name.clone()).into_any_element(), None, cx))
                            .child(self.labelled("Comments", div().debug_selector(|| "field-comments".to_string()).child(f.comment.clone()).into_any_element(), None, cx))
                            .children(f.sections.iter().enumerate().map(|(si, s)| self.section_view(lp, id, si, s, cx)))
                    })
                    .when(form.is_some_and(|f| f.sections.is_empty()), |d| d.child(div().text_color(p.muted).child("This element has no other settings.")))
                    .when(form.is_none(), |d| d.child(div().text_color(p.muted).child("Loading\u{2026}"))),
            )
            .into_any_element()
    }

    fn enabled_button(&self, enabled: bool, cx: &mut Context<Self>) -> AnyElement {
        div()
            .debug_selector(|| "toggle-enabled".to_string())
            .child(
                Button::new("toggle-enabled", if enabled { "Enabled" } else { "Disabled" })
                    .style(if enabled { ButtonStyle::Subtle } else { ButtonStyle::Tinted(ui::TintColor::Warning) })
                    .size(ButtonSize::Compact)
                    .tooltip(Tooltip::text("Switch this element on or off (Ctrl+T in the tree)"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.flush_form(window, cx);
                        this.toggle_selected_enabled(window, cx);
                    })),
            )
            .into_any_element()
    }

    /// A label on the left and a control on the right, wrapping on a narrow window.
    fn labelled(&self, label: &str, control: AnyElement, help: Option<&str>, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(8.))
            .child(div().w(px(LABEL_WIDTH)).max_w_full().flex_none().pt(px(5.)).text_size(px(12.5)).text_color(p.muted).child(label.to_string()))
            .child(div().flex_1().min_w(px(200.)).max_w(px(560.)).flex().flex_col().gap(px(3.)).child(control).when_some(help.map(str::to_string), |d, h| d.child(div().text_size(px(11.5)).text_color(p.muted).child(h))))
            .into_any_element()
    }

    fn section_view(&self, lp: &LoadedPlan, node: NodeId, si: usize, s: &SectionForm, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        div()
            .id(("section", si))
            .flex()
            .flex_col()
            .gap(px(8.))
            .mt(px(6.))
            .child(div().pb(px(2.)).border_b_1().border_color(p.line).text_size(px(11.)).font_family(MONO_FONT).text_color(p.muted).child(s.title.to_uppercase()))
            .children(s.items.iter().map(|item| self.item_view(lp, node, item, cx)))
            .children(s.tables.iter().map(|t| self.table_view(t, cx)))
            .into_any_element()
    }

    fn item_view(&self, lp: &LoadedPlan, node: NodeId, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let def = item.def.clone();
        let control: AnyElement = match &item.widget {
            Widget::Text(f) => {
                let field = div().flex_1().min_w_0().debug_selector({
                    let id = format!("field-{}", def.prop);
                    move || id.clone()
                }).child(f.clone());
                let browse = lp.plan.node(node).is_some_and(|n| crate::run_props::browse_target(&n.tag, &def.prop));
                if browse {
                    let prop = def.prop.clone();
                    let id = format!("browse-{}", def.prop);
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(field)
                        .child(div().debug_selector({
                            let id = id.clone();
                            move || id.clone()
                        }).child(Button::new(SharedString::from(id), "Browse\u{2026}").style(ButtonStyle::Subtle).size(ButtonSize::Compact).on_click(cx.listener(move |this, _, window, cx| this.browse_for_file(prop.clone(), window, cx)))))
                        .into_any_element()
                } else {
                    field.into_any_element()
                }
            }
            Widget::Check => {
                let cur = lp.edit.prop(node, &def.prop).unwrap_or_else(|| def.default.clone());
                let on = match def.kind {
                    Kind::BitFlag(bit) => cur.trim().parse::<i64>().unwrap_or(0) & bit != 0,
                    _ => cur == "true",
                };
                let id = match def.kind {
                    Kind::BitFlag(bit) => format!("check-{}#{bit}", def.prop),
                    _ => format!("check-{}", def.prop),
                };
                let handler = cx.listener({
                    let def = def.clone();
                    move |this, _, window, cx| {
                        this.flush_form(window, cx);
                        this.toggle_setting(&def, window, cx);
                    }
                });
                self.checkbox(id.clone(), on, cx, handler)
            }
            Widget::Combo(c) => div().child(c.clone()).into_any_element(),
            Widget::Area(a) => {
                // Several lines need the width of the page: the label goes above, not beside.
                let id = format!("area-{}", def.prop);
                return div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(div().text_size(px(12.5)).text_color(p.muted).child(def.label.clone()))
                    .child(div().debug_selector(move || id.clone()).max_w(px(900.)).child(a.clone()))
                    .into_any_element();
            }
            Widget::ReadOnly(text) => div().px(px(8.)).py(px(4.)).rounded(px(6.)).bg(p.panel).border_1().border_color(p.line).text_color(p.muted).font_family(MONO_FONT).text_size(px(12.)).child(if text.is_empty() { "\u{2014}".to_string() } else { text.clone() }).into_any_element(),
        };
        let help = if matches!(item.widget, Widget::ReadOnly(_)) && matches!(item.def.kind, Kind::Summary) { Some("A list or group of settings: change it in JMeter for now.") } else { None };
        self.labelled(&def.label, control, help, cx)
    }

    fn checkbox(&self, id: String, on: bool, cx: &mut Context<Self>, on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static) -> AnyElement {
        let p = theme::palette(cx);
        div()
            .id(gpui::SharedString::from(id.clone()))
            .debug_selector(move || id.clone())
            .w(px(18.))
            .h(px(18.))
            .mt(px(3.))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .border_1()
            .border_color(if on { p.accent } else { p.line })
            .bg(if on { p.accent } else { p.bg })
            .cursor_pointer()
            .hover(|d| d.border_color(p.accent))
            .on_click(on_click)
            .when(on, |d| d.child(icon("icons/check.svg", 13., p.on_accent)))
            .into_any_element()
    }

    fn table_view(&self, t: &TableForm, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let path = t.path.clone();
        let shape = t.shape;
        let widths: Vec<f32> = t.columns.iter().map(|c| if c.kind == Kind::Bool { 90. } else { 0. }).collect();
        let header = div().flex().items_center().gap(px(8.)).px(px(8.)).py(px(4.)).bg(p.panel).text_size(px(11.)).font_family(MONO_FONT).text_color(p.muted).children(t.columns.iter().zip(&widths).map(|(c, w)| {
            let d = div().child(c.label.to_uppercase());
            if *w > 0. { d.w(px(*w)).flex_none() } else { d.flex_1().min_w(px(100.)) }
        })).child(div().w(px(28.)).flex_none());
        let rows = t.rows.iter().enumerate().map(|(ri, row)| {
            let path = path.clone();
            div()
                .id(("table-row", ri))
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .py(px(3.))
                .border_t_1()
                .border_color(p.line)
                .children(row.iter().zip(&widths).enumerate().map(|(ci, (cell, w))| {
                    let cell_el: AnyElement = match cell {
                        Cell::Text(f) => div().debug_selector({
                            let id = format!("cell-{}-{ri}-{ci}", path);
                            move || id.clone()
                        }).child(f.clone()).into_any_element(),
                        Cell::Check { prop, on } => {
                            let id = format!("cellcheck-{path}-{ri}-{prop}");
                            let (path, prop) = (path.clone(), prop.clone());
                            let handler = cx.listener(move |this, _, window, cx| {
                                this.flush_form(window, cx);
                                this.toggle_cell(&path, ri, &prop, window, cx);
                            });
                            self.checkbox(id, *on, cx, handler)
                        }
                    };
                    let d = div().child(cell_el);
                    if *w > 0. { d.w(px(*w)).flex_none().flex().justify_center() } else { d.flex_1().min_w(px(100.)) }
                }))
                .child(
                    div().w(px(28.)).flex_none().child(
                        IconButton::new(("row-remove", ri), IconName::Close)
                            .icon_size(IconSize::XSmall)
                            .tooltip(Tooltip::text("Remove this row"))
                            .on_click(cx.listener({
                                let path = path.clone();
                                move |this, _, window, cx| {
                                    this.flush_form(window, cx);
                                    this.remove_table_row(&path, ri, window, cx);
                                }
                            })),
                    ),
                )
        });
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(div().text_size(px(12.5)).text_color(p.muted).child(t.title.clone()))
            .child(div().border_1().border_color(p.line).rounded(px(8.)).overflow_hidden().max_w(px(760.)).child(header).children(rows).when(t.rows.is_empty(), |d| d.child(div().px(px(10.)).py(px(8.)).border_t_1().border_color(p.line).text_color(p.muted).text_size(px(12.)).child("Nothing here yet."))))
            .child(
                div().flex().child(
                    div().debug_selector({
                        let id = format!("add-row-{}", t.path);
                        move || id.clone()
                    }).child(
                        Button::new(gpui::SharedString::from(format!("add-row-{}", t.path)), t.add_label.clone())
                            .style(ButtonStyle::Subtle)
                            .size(ButtonSize::Compact)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.flush_form(window, cx);
                                this.add_table_row(&path, shape, window, cx);
                            })),
                    ),
                ),
            )
            .into_any_element()
    }
}
