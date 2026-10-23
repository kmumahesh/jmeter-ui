//! A drop-down list: shows the chosen option and opens a menu of the others.

use crate::theme;
use gpui::{Anchor, Context, EventEmitter, InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px};
use ui::{Button, ButtonCommon, ButtonSize, ButtonStyle, ContextMenu, ContextMenuEntry, ContextMenuItem, IconName, IconPosition, PopoverMenu};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComboEvent {
    /// The user (or a call to `select`) chose the option at this index.
    Selected(usize),
}

pub struct ComboBox {
    id: SharedString,
    options: Vec<SharedString>,
    selected: usize,
}

impl EventEmitter<ComboEvent> for ComboBox {}

impl ComboBox {
    /// `selected` is clamped to a valid index; an empty option list is allowed but shows nothing to pick.
    pub fn new(id: impl Into<SharedString>, options: &[&str], selected: usize) -> ComboBox {
        let options: Vec<SharedString> = options.iter().map(|s| SharedString::from(s.to_string())).collect();
        let selected = selected.min(options.len().saturating_sub(1));
        ComboBox { id: id.into(), options, selected }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_label(&self) -> &str {
        self.options.get(self.selected).map(|s| s.as_ref()).unwrap_or("")
    }

    pub fn len(&self) -> usize {
        self.options.len()
    }

    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    /// Choose an option. Does nothing for an index that does not exist or is already chosen.
    pub fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.options.len() || index == self.selected {
            return;
        }
        self.selected = index;
        cx.emit(ComboEvent::Selected(index));
        cx.notify();
    }

    /// Change the shown option without telling listeners (for loading a saved value).
    pub fn set_selected_silently(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.options.len() {
            self.selected = index;
            cx.notify();
        }
    }
}

impl Render for ComboBox {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let combo = cx.entity();
        let options = self.options.clone();
        let selected = self.selected;
        let id = self.id.clone();
        let label = self.selected_label().to_string();
        div().debug_selector({
            let id = id.to_string();
            move || id.clone()
        })
        .child(
            PopoverMenu::new(id.clone())
                .anchor(Anchor::TopLeft)
                .menu(move |window, cx| {
                    let combo = combo.clone();
                    let options = options.clone();
                    Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                        for (i, label) in options.iter().enumerate() {
                            let combo = combo.clone();
                            let entry = ContextMenuEntry::new(label.clone()).toggleable(IconPosition::End, i == selected).handler(move |_, cx| {
                                combo.update(cx, |c, cx| c.select(i, cx));
                            });
                            menu = menu.item(ContextMenuItem::Entry(entry));
                        }
                        menu
                    }))
                })
                .trigger(Button::new(id, label).style(ButtonStyle::Outlined).size(ButtonSize::Default).end_icon(ui::Icon::new(IconName::ChevronDown).size(ui::IconSize::XSmall))),
        )
        .text_color(p.fg)
        .max_w(px(360.))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_is_clamped_to_the_options() {
        assert_eq!(ComboBox::new("c", &["a", "b"], 5).selected(), 1);
        assert_eq!(ComboBox::new("c", &["a", "b"], 0).selected_label(), "a");
        let empty = ComboBox::new("c", &[], 3);
        assert!(empty.is_empty());
        assert_eq!(empty.selected_label(), "");
    }
}
