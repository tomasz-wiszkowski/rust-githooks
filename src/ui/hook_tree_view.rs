use crate::hooks::Action;
use crate::hooks::Hook;
use crate::hooks::Hooks;
use anyhow::Result;

use tui::{
    style::{Color, Modifier, Style},
    widgets::{List, ListItem},
};

enum ListElement {
    Space(ListItem<'static>),
    Category(ListItem<'static>),
    Action(ListItem<'static>, Action),
}

impl From<&ListElement> for ListItem<'static> {
    fn from(value: &ListElement) -> Self {
        match value {
            ListElement::Space(e) => e,
            ListElement::Category(e) => e,
            ListElement::Action(e, _) => e,
        }
        .clone()
    }
}

pub struct HooksTreeView {
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

    fn action_tree_node(action: Action) -> ListElement {
        let marker = if !action.borrow().is_selected() {
            ' '
        } else if !action.borrow().is_available() {
            '✘'
        } else {
            '✔'
        };

        ListElement::Action(
            ListItem::new(format!("    [{}] {}", marker, action.borrow().name())),
            action.clone(),
        )
    }

    fn get_actions_sorted_by_name(hook: &Hook) -> Vec<Action> {
        let mut res = hook
            .actions()
            .iter()
            .map(|(_, a)| a.clone())
            .collect::<Vec<_>>();
        res.sort_by(|a, b| a.borrow().name().cmp(b.borrow().name()));
        res
    }

    fn build_items_list(hooks: &Hooks) -> Vec<ListElement> {
        let mut items = vec![];

        for (_, hook) in hooks.iter() {
            items.push(Self::space_tree_node());
            items.push(Self::category_tree_node(hook));

            Self::get_actions_sorted_by_name(hook)
                .into_iter()
                .for_each(|a| items.push(Self::action_tree_node(a)));
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
                ListElement::Category(_) | ListElement::Action(_, _) => {
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
                ListElement::Category(_) | ListElement::Action(_, _) => {
                    self.selected = next_item;
                    break;
                }
                _ => next_item += 1,
            }
        }
    }

    pub fn toggle_selected(&mut self) -> Result<()> {
        let ListElement::Action(_, action) = self.items.get(self.selected).unwrap() else {
            return Ok(());
        };

        let action_selected = action.borrow().is_selected();
        action.borrow_mut().set_selected(!action_selected)?;

        self.items[self.selected] = Self::action_tree_node(action.clone());
        Ok(())
    }

    pub fn new(hooks: Hooks) -> Self {
        let style_deselected = Style::default().fg(Color::DarkGray);
        let style_selected = Style::default().add_modifier(Modifier::BOLD);
        let items = Self::build_items_list(&hooks);
        Self {
            items,
            selected: 0usize,
            style_selected,
            style_deselected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::Actions;
    use crate::hooks::Hook;
    use crate::hooks::Hooks;
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::hooks::ShellAction;

    fn create_mock_hooks() -> Hooks {
        // Create mock hooks and actions for testing
        // This is a simplified version and may need to be adjusted based on your actual implementation
        let action1 = Rc::new(RefCell::new(ShellAction::new_for_test("Action 1")));
        let mut actions1 = Actions::new();
        actions1.insert("action1".into(), action1);

        let action2 = Rc::new(RefCell::new(ShellAction::new_for_test("Action 2")));
        let mut actions2 = Actions::new();
        actions2.insert("action2".into(), action2);

        let hook1 = Hook::new("1".into(), "Hook 1".into(), actions1);
        let hook2 = Hook::new("2".into(), "Hook 2".into(), actions2);

        let mut hooks = Hooks::new();

        hooks.insert(hook1.id().into(), hook1);
        hooks.insert(hook2.id().into(), hook2);

        hooks
    }

    #[test]
    fn test_new() {
        let hooks = create_mock_hooks();
        let tree_view = HooksTreeView::new(hooks);

        assert_eq!(tree_view.selected, 0);
        assert_eq!(tree_view.items.len(), 6); // 2 spaces, 2 categories, 2 actions
    }

    #[test]
    fn test_select_next_item() {
        let hooks = create_mock_hooks();
        let mut tree_view = HooksTreeView::new(hooks);

        tree_view.select_next_item();
        assert_eq!(tree_view.selected, 1);

        tree_view.select_next_item();
        assert_eq!(tree_view.selected, 2);
    }

    #[test]
    fn test_select_prev_item() {
        let hooks = create_mock_hooks();
        let mut tree_view = HooksTreeView::new(hooks);

        tree_view.selected = 3;
        tree_view.select_prev_item();
        assert_eq!(tree_view.selected, 2);

        tree_view.select_prev_item();
        assert_eq!(tree_view.selected, 1); // Should not go below 1
    }

    /*
    #[test]
    fn test_toggle_selected() {
        let hooks = create_mock_hooks();
        let mut tree_view = HooksTreeView::new(hooks);

        tree_view.selected = 2; // Select an action
        tree_view.toggle_selected().unwrap();

        if let ListElement::Action(_, action) = &tree_view.items[2] {
            assert!(action.borrow().is_selected());
        } else {
            panic!("Expected Action at index 2");
        }

        tree_view.toggle_selected().unwrap();

        if let ListElement::Action(_, action) = &tree_view.items[2] {
            assert!(!action.borrow().is_selected());
        } else {
            panic!("Expected Action at index 2");
        }
    }
    */
}
