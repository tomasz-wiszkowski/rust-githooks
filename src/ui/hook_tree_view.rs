use anyhow::Result;
use anyhow::bail;
use tui::{
    widgets::{List, ListItem},
    style::{Color, Style},
};
use crate::hooks::hook::Hook;
use crate::hooks::hooks::Hooks;
use crate::hooks::shell_action::ShellAction;

pub struct HookTreeNodeData {
    hook: Option<Box<Hook>>,
    action: Option<Box<ShellAction>>,
}

pub struct HooksTreeView {
    hooks: Hooks,
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

    fn update_tree_node(&self, action: &Box<dyn Action>, node: &mut TreeItem) {
        let marker = if !action.is_selected() {
            ' '
        } else if !action.is_available() {
            '✘'
        } else {
            '✔'
        };

        node.text = format!("[{}] {}", marker, action.name());
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
    pub fn widget(&self) -> List {
        let mut items = vec![];

        for (_, hook) in self.hooks.iter() {
            let hook_item = ListItem::new(format!("[ ] {}", hook.name()));
            items.push(hook_item);

            for action in hook.actions().iter() {
                let action_item = ListItem::new(format!("    [ ] {}", action.name()));
                items.push(action_item);
            }
        }

        List::new(items)
    }

    pub fn new(hooks: Hooks) -> Self {
        Self { hooks }
    }
}


