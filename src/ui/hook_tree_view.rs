use anyhow::Result;
use anyhow::bail;
use tui::{
    widgets::{Block, Borders, List, ListItem},
    style::{Color, Style, Modifier},
};
use crate::hooks::hook::Hook;
use crate::hooks::hooks::Hooks;
use crate::hooks::shell_action::ShellAction;

pub struct HookTreeNodeData {
    hook: Option<Box<Hook>>,
    action: Option<Box<ShellAction>>,
}

enum ListElement {
    Space(ListItem<'static>),
    Category(ListItem<'static>, String),
    Action(ListItem<'static>, String, usize)
}

impl From<&ListElement> for ListItem<'static> {
    fn from(value: &ListElement) -> Self {
        match value {
            ListElement::Space(e) => e,
            ListElement::Category(e, _) => e,
            ListElement::Action(e, _, _) => e,
        }.clone()
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
    /*
    fn add_hook_tree_nodes(&mut self, target: &mut TreeItem) {
        let mut hks: Vec<Box<dyn Hook>> = self.data.values().cloned().collect();
        hks.sort_by(|a, b| a.name().cmp(b.name()));

        for c in hks {
            let node = TreeItem::new(c.name())
                .data(Box::new(HookTreeNodeData { hook: Some(c), action: None }))
                .style(Style::default().fg(Color::Gray));
            target.add_child(node);
            self.add(&mut target.children.last_mut().unwrap(), &target.data.as_ref().unwrap().downcast_ref::<HookTreeNodeData>().unwrap());
        }
    }

    fn add_action_tree_nodes(&mut self, target: &mut TreeItem, ref_: &HookTreeNodeData) {
        let mut actions = ref_.hook.as_ref().unwrap().actions();
        actions.sort_by(|a, b| a.name().cmp(b.name()));

        for h in actions {
            let mut node = TreeItem::new("")
                .data(Box::new(HookTreeNodeData { hook: ref_.hook.clone(), action: Some(h.clone()) }));
            self.update_tree_node(&h, &mut node);
            target.add_child(node);
        }
    }

    fn add(&mut self, target: &mut TreeItem, ref_: &HookTreeNodeData) {
        if ref_.hook.is_none() {
            self.add_hook_tree_nodes(target);
        } else if ref_.action.is_none() {
            self.add_action_tree_nodes(target, ref_);
        }
    }

    fn on_tree_node_selected(&mut self, node: &mut TreeItem) {
        let reference = node.data.as_ref().unwrap().downcast_ref::<HookTreeNodeData>().unwrap();

        if let Some(action) = &reference.action {
            action.set_selected(!action.is_selected());
            self.update_tree_node(action, node);
        } else {
            node.is_expanded = !node.is_expanded;
        }
    }
    pub fn new(data: Hooks) -> Result<Self> {
        let root = TreeItem::new("Hooks").style(Style::default().fg(Color::Gray));

        let mut view = HooksTreeView {
            tree: Tree::new(vec![root.clone()]),
            root,
            data,
        };
        view.add(&mut view.root, &HookTreeNodeData { hook: None, action: None });

        Ok(view)
    }
 */
    fn space_tree_node() -> ListElement {
        ListElement::Space(ListItem::new(" "))
    }

    fn category_tree_node(hook: &Hook) -> ListElement {
        ListElement::Category(ListItem::new(format!("{} - {}", hook.id(), hook.name())), hook.id().into())
    }

    fn action_tree_node(category_id: &str, index: usize, action: &ShellAction) -> ListElement {
        let marker = if !action.is_selected() {
            ' '
        } else if !action.is_available() {
            '✘'
        } else {
            '✔'
        };

        ListElement::Action(ListItem::new(format!("    [{}] {}", marker, action.name())), category_id.into(), index)
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
                ListElement::Category(_, _) | ListElement::Action(_, _, _) => {
                    self.selected = next_item as usize;
                    break;
                },
                _ => next_item -= 1
            }
        }
    }

    pub fn select_next_item(&mut self) {
        let mut next_item = self.selected + 1;
        while next_item < self.items.len() {
            match self.items.get(next_item).unwrap() {
                ListElement::Category(_, _) | ListElement::Action(_, _, _) => {
                    self.selected = next_item;
                    break;
                },
                _ => next_item += 1
            }
        }
    }

    pub fn toggle_selected(&mut self) {
        let ListElement::Action(_, category_id, index) = self.items.get(self.selected).unwrap() else {
            return;
        };

        let index = *index;

        let Some(category) = self.hooks.get_mut(category_id) else {
            return;
        };

        let Some(action) = category.actions_mut().get_mut(index) else {
            return;
        };

        action.set_selected(!action.is_selected());

        self.items[self.selected] = Self::action_tree_node(category_id, index, action);
    }

    pub fn new(hooks: Hooks) -> Self {
        let style_deselected = Style::default().fg(Color::DarkGray);
        let style_selected = Style::default().add_modifier(Modifier::BOLD);
        let items = Self::build_items_list(&hooks);
        Self { hooks, items, selected: 0usize, style_selected, style_deselected }
    }
}


