use anyhow::Result;

use std::error::Error;
use git2::{Repository, Config, Delta};
use std::path::Path;
use std::rc::Rc;

use crate::config::GitConfigManager;

struct GitRepo {
    repo: Rc<Repository>,
    config: GitConfigManager,
}

fn git_repo_open() -> Result<GitRepo> {
    let repo = Rc::new(Repository::open_from_env()?);
    let config = GitConfigManager::new(&repo)?;

    Ok(GitRepo {
        repo,
        config,
    })
}

impl GitRepo {
    fn work_dir(&self) -> Option<&Path> {
        self.repo.workdir()
    }

    fn config_dir(&self) -> &Path {
        self.repo.path()
    }

    fn get_list_of_new_and_modified_files(&self) -> Result<Vec<String>> {
        let head = self.repo.head()?;
        let commit = self.repo.find_commit(head.target().unwrap())?;
        let parent = match commit.parent(0) {
            Ok(parent) => parent,
            Err(_) => {
                println!("Unable to query parent commit - assuming first commit");
                return self.get_list_of_all_files();
            }
        };

        let tree1 = commit.tree().ok();
        let tree2 = parent.tree().ok();

        let changes = self.repo.diff_tree_to_tree(tree2.as_ref(), tree1.as_ref(), None)?;

        let mut paths = Vec::new();
        for delta in changes.deltas() {
            let action = delta.status();
            match action {
                Delta::Deleted => continue,
                Delta::Added | Delta::Modified => {
                    paths.push(delta.new_file().path().unwrap().to_string_lossy().to_string())
                }
                _ => {}
            }
        }

        Ok(paths)
    }

    fn get_list_of_all_files(&self) -> Result<Vec<String>> {
        let tree = self.repo.head()?.peel_to_tree()?;
        let mut paths = Vec::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |_, entry| {
            paths.push(entry.name().unwrap().to_string());
            0
        })?;
        Ok(paths)
    }
}
