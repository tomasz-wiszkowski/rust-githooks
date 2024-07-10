use crate::hooks::hook::Hook;
use crate::hooks::hooks::Hooks;
use crate::hooks::shell_action::ShellAction;
use anyhow::Result;

use tui::{
    style::{Color, Modifier, Style},
    widgets::{List, ListItem},
};

enum ListElement {
    Space(ListItem<'static>),
    Category(ListItem<'static>),
    Action(ListItem<'static>, String, usize),
}

impl From<&ListElement> for ListItem<'static> {
    fn from(value: &ListElement) -> Self {
        match value {
            ListElement::Space(e) => e,
            ListElement::Category(e) => e,
            ListElement::Action(e, _, _) => e,
        }
        .clone()
    }
}

pub struct HooksTreeView {
    hooks: Hooks,

    items: Vec<ListElement>,
    selected: usize,

    style_selected: Style,
    style_deselected: Style,
}

impl HooksTreeView {
    fn space_tree_node() -> ListElement {
        ListElement::Space(ListItem::new(" "))
    }

    fn category_tree_node(hook: &Hook) -> ListElement {
        ListElement::Category(ListItem::new(format!("{} - {}", hook.id(), hook.name())))
    }

    fn action_tree_node(category_id: &str, index: usize, action: &ShellAction) -> ListElement {
        let marker = if !action.is_selected() {
            ' '
        } else if !action.is_available() {
            '✘'
        } else {
            '✔'
        };

        ListElement::Action(
            ListItem::new(format!("    [{}] {}", marker, action.name())),
            category_id.into(),
            index,
        )
    }

    fn build_items_list(hooks: &Hooks) -> Vec<ListElement> {
        let mut items = vec![];

        for (_, hook) in hooks.iter() {
            items.push(Self::space_tree_node());
            items.push(Self::category_tree_node(hook));

            for (index, action) in hook.actions().iter().enumerate() {
                items.push(Self::action_tree_node(hook.id(), index, action));
            }
        }

        items
    }

    pub fn widget(&self) -> List {
        let mut items = vec![];

        for (index, elem) in self.items.iter().enumerate() {
            let mut item = ListItem::from(elem);
            if index == self.selected {
                item = item.style(self.style_selected);
            } else {
                item = item.style(self.style_deselected);
            }
            items.push(item);
        }
        List::new(items)
    }

    pub fn select_prev_item(&mut self) {
        let mut next_item = (self.selected as i32) - 1;
        while next_item >= 0 {
            match self.items.get(next_item as usize).unwrap() {
                ListElement::Category(_) | ListElement::Action(_, _, _) => {
                    self.selected = next_item as usize;
                    break;
                }
                _ => next_item -= 1,
            }
        }
    }

    pub fn select_next_item(&mut self) {
        let mut next_item = self.selected + 1;
        while next_item < self.items.len() {
            match self.items.get(next_item).unwrap() {
                ListElement::Category(_) | ListElement::Action(_, _, _) => {
                    self.selected = next_item;
                    break;
                }
                _ => next_item += 1,
            }
        }
    }

    pub fn toggle_selected(&mut self) -> Result<()> {
        let ListElement::Action(_, category_id, index) = self.items.get(self.selected).unwrap()
        else {
            return Ok(());
        };

        let index = *index;

        let Some(category) = self.hooks.get_mut(category_id) else {
            return Ok(());
        };

        let Some(action) = category.actions_mut().get_mut(index) else {
            return Ok(());
        };

        action.set_selected(!action.is_selected())?;

        self.items[self.selected] = Self::action_tree_node(category_id, index, action);
        Ok(())
    }

    pub fn new(hooks: Hooks) -> Self {
        let style_deselected = Style::default().fg(Color::DarkGray);
        let style_selected = Style::default().add_modifier(Modifier::BOLD);
        let items = Self::build_items_list(&hooks);
        Self {
            hooks,
            items,
            selected: 0usize,
            style_selected,
            style_deselected,
        }
    }
}
