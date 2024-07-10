// mod check;
mod hooks;
mod repo;
mod ui;

use std::env;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use crossterm::terminal::disable_raw_mode;
use hooks::hooks::Hooks;
use hooks::hooks::HooksExt;
use repo::repo::GitRepo;
use ui::hook_tree_view::HooksTreeView;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode};
use tui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem},
    Terminal,
};


struct Data {
    repo: GitRepo,
    hooks: Hooks,
}

fn open_repo() -> Result<Data> {
    let repo = GitRepo::new()?;
    let mut hooks = hooks::hooks::get_hooks();
    hooks.set_config_store(repo.get_config_manager());
    Ok(Data{ repo, hooks })
}

fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let self_name = Path::new(&args[0]).file_name().unwrap().to_str().unwrap();
    let data = open_repo()?;

    if data.hooks.contains_key(self_name) {
        let args = Vec::from(&args[1..]);
        run_hooks(data, self_name, args)
    } else if args.len() == 1 {
        show_config(data)
    } else if data.hooks.contains_key(&args[1]) {
        let hook_name = &args[1];
        let args = Vec::from(&args[2..]);
        run_hooks(data, hook_name, args)
    } else if args[1] == "install" {
        install(data)
    } else {
        anyhow::bail!("Unknown hook type {}", args[1]);
    }
}

fn run_hooks(data: Data, hook_name: &str, args: Vec<String>) -> Result<()> {
    let hook = data.hooks.get(hook_name).context(format!("Find hook for {}", hook_name))?;
    let files = data.repo.get_list_of_new_and_modified_files()?;

    env::set_current_dir(data.repo.work_dir().context("Could not get workdir root")?)
        .context("Run: cannot open work directory")?;

    let actions = hook.actions();
    for h in actions {
        h.run(&files, &args)?;
    }

    Ok(())
}

fn show_config(data: Data) -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut tree = HooksTreeView::new(data.hooks);

    loop {
        terminal.draw(|f| {
            let size = f.size();
            f.render_widget(tree.widget(), size);

        })?;

        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Esc {
                break;
            }

            if key.code == KeyCode::Up {
                tree.select_prev_item();
            }

            if key.code == KeyCode::Down {
                tree.select_next_item();
            }

            if key.code == KeyCode::Char(' ') {
                tree.toggle_selected();
            }
        }
    }

    terminal.clear();
    crossterm::terminal::disable_raw_mode()?;
    Ok(())
}

fn install(data: Data) -> Result<()> {
    let self_absolute_path = env::current_exe().context("Install: cannot locate self")?;

    let config_dir = data.repo.config_dir();

    fs::create_dir_all(config_dir.join("hooks")).context("Install: failed to create hooks directory")?;

    let hook_dir = config_dir.join("hooks");

        for (_, hook) in data.hooks.iter() {
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

    show_config(data)
}

