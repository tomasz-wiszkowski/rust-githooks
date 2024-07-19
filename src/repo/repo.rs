use anyhow::Result;

use git2::{Delta, Repository};
use std::path::Path;
use std::rc::Rc;

use super::config::GitConfigManager;
use super::config::GitConfigManagerImpl;

pub struct GitRepo {
    repo: Rc<Repository>,
    config: Box<dyn GitConfigManager>,
}

impl GitRepo {
    pub fn new() -> Result<GitRepo> {
        let repo = Rc::new(Repository::open_from_env()?);
        let config = GitConfigManagerImpl::new(&repo)?;

        Ok(GitRepo { repo, config })
    }

    pub fn work_dir(&self) -> Option<&Path> {
        self.repo.workdir()
    }

    pub fn config_dir(&self) -> &Path {
        self.repo.path()
    }

    pub fn get_config_manager(&self) -> &Box<dyn GitConfigManager> {
        &self.config
    }

    pub fn get_list_of_new_and_modified_files(&self) -> Result<Vec<String>> {
        let head = self.repo.head()?;
        let commit = self.repo.find_commit(head.target().unwrap())?;
        let parent = match commit.parent(0) {
            Ok(parent) => parent,
            Err(_) => {
                log::info!("Unable to query parent commit - assuming first commit");
                return self.get_list_of_all_files();
            }
        };

        let tree1 = commit.tree().ok();
        let tree2 = parent.tree().ok();

        let changes = self
            .repo
            .diff_tree_to_tree(tree2.as_ref(), tree1.as_ref(), None)?;

        let mut paths = Vec::new();
        for delta in changes.deltas() {
            let action = delta.status();
            match action {
                Delta::Deleted => continue,
                Delta::Added | Delta::Modified => paths.push(
                    delta
                        .new_file()
                        .path()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                ),
                _ => {}
            }
        }

        Ok(paths)
    }

    pub fn get_list_of_all_files(&self) -> Result<Vec<String>> {
        let tree = self.repo.head()?.peel_to_tree()?;
        let mut paths = Vec::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |entry_path, entry| {
            let p = Path::new(entry_path);
            if let Some(name) = entry.name() {
                let p = p.join(name);
                p.to_str().map(|s| paths.push(s.to_owned()));
            }
            0
        })?;
        Ok(paths)
    }
}
