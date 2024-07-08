// mod check;
mod hooks;
mod repo;
mod ui;

use std::env;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use anyhow::bail;
use hooks::hook::Hook;
use hooks::hooks::HooksExt;
use repo::repo::GitRepo;
use std::cell::Ref;
use ui::hook_tree_view::HooksTreeView;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode};
use tui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};


fn open_repo() -> Result<GitRepo> {
    let r = GitRepo::new()?;
    hooks::hooks::get_hooks().borrow_mut().set_config_store(r.get_config_manager());
    Ok(r)
}

fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let self_name = Path::new(&args[0]).file_name().unwrap().to_str().unwrap();
    let hks = hooks::hooks::get_hooks();

    if let Some(h) = hks.borrow().get(self_name) {
        let args = Vec::from(&args[1..]);
        run_hooks(h, args)
    } else if args.len() == 1 {
        show_config()
    } else if let Some(h) = hks.borrow().get(&args[1]) {
        let args = Vec::from(&args[2..]);
        run_hooks(h, args)
    } else if args[1] == "install" {
        install()
    } else {
        anyhow::bail!("Unknown hook type {}", args[1]);
    }
}

fn run_hooks(hook: &Hook, args: Vec<String>) -> Result<()> {
    let repo = open_repo()?;
    let files = repo.get_list_of_new_and_modified_files()?;

    env::set_current_dir(repo.work_dir().context("Could not get workdir root")?)
        .context("Run: cannot open work directory")?;

    let actions = hook.actions();
//    actions.sort_by_key(|a| a.priority());
    for h in actions {
        h.run(&files, &args)?;
    }

    Ok(())
}

fn show_config() -> Result<()> {
    let repo = open_repo()?;

    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

//    let tree = HooksTreeView::new(hooks::hooks::get_hooks())?;

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let block = Block::default().title("Hooks").borders(Borders::ALL);
            f.render_widget(block, size);
            /*
            let items: Vec<ListItem> = tree
                .items()
                .iter()
                .map(|i| ListItem::new(i.to_string()))
                .collect();
            let list = List::new(items).block(Block::default().borders(Borders::NONE));
            f.render_widget(list, size);
            */
        })?;

        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Esc {
                break;
            }
        }
    }

    Ok(())
}

fn install() -> Result<()> {
    let self_absolute_path = env::current_exe().context("Install: cannot locate self")?;

    let repo = open_repo()?;
    let config_dir = repo.config_dir();

    fs::create_dir_all(config_dir.join("hooks")).context("Install: failed to create hooks directory")?;

    let hook_dir = config_dir.join("hooks");

    let hooks = hooks::hooks::get_hooks();
        for (_, hook) in hooks.borrow().iter() {
            println!(
                "Installing {} in {} pointing to {}",
                hook.id(),
                hook_dir.display(),
                self_absolute_path.display()
            );

            let hook_path = hook_dir.join(hook.id());
            if hook_path.exists() {
                fs::remove_file(&hook_path).context(format!("Install: failed to remove hook {}", hook.name()))?;
            }

            unix_fs::symlink(&self_absolute_path, &hook_path)
                .context(format!("Install: failed to install hook {}", hook.name()))?;
        }

    show_config()
}

