use anyhow::Result;

use git2::{Delta, DiffOptions, Repository};
use git2::{FileMode, ObjectType};
use log::info;
use std::path::Path;
use std::rc::Rc;

use super::GitConfigManager;
use super::GitConfigManagerImpl;
use super::Item;

/// Represents a Git repository.
pub struct GitRepo {
    repo: Rc<Repository>,
    config: Box<dyn GitConfigManager>,
}

impl GitRepo {
    /// Creates a new instance of `GitRepo`.
    ///
    /// # Returns
    /// A `Result` containing the `GitRepo` instance or an error.
    pub fn new() -> Result<GitRepo> {
        let repo = Rc::new(Repository::open_from_env()?);
        let config = GitConfigManagerImpl::new(&repo)?;

        Ok(GitRepo { repo, config })
    }

    /// Returns the working directory of the repository.
    ///
    /// # Returns
    /// An `Option` containing a reference to the working directory path.
    pub fn work_dir(&self) -> Option<&Path> {
        self.repo.workdir()
    }

    /// Returns the configuration directory of the repository.
    ///
    /// # Returns
    /// A reference to the configuration directory path.
    pub fn config_dir(&self) -> &Path {
        self.repo.path()
    }

    /// Returns a reference to the configuration manager.
    ///
    /// # Returns
    /// A reference to the `GitConfigManager`.
    pub fn get_config_manager(&self) -> &Box<dyn GitConfigManager> {
        &self.config
    }

    /// Retrieves the items that are staged for a pre-commit.
    ///
    /// # Returns
    /// A `Result` containing a vector of `Item` or an error.
    pub fn get_pre_commit_items(&self) -> Result<Vec<Item>> {
        info!("Querying pre-commit items");
        let Ok(head) = self.repo.head() else {
            info!("Could not evaluate HEAD - assuming new repository.");
            return Ok(Vec::default());
        };

        let commit = self.repo.find_commit(head.target().unwrap())?;

        let tree = commit.tree().ok();
        let changes = self.repo.diff_tree_to_index(
            tree.as_ref(),
            None,
            Some(DiffOptions::new().ignore_submodules(false)),
        )?;

        let mut paths = Vec::new();
        for delta in changes.deltas() {
            let action = delta.status();
            match action {
                Delta::Deleted => continue,

                Delta::Added | Delta::Modified => {
                    let item_path = delta
                        .new_file()
                        .path()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();

                    paths.push(match delta.new_file().mode() {
                        FileMode::Unreadable => Item::Unknown(item_path),
                        FileMode::Blob | FileMode::BlobGroupWritable | FileMode::BlobExecutable => {
                            Item::File(item_path)
                        }
                        FileMode::Tree => Item::Dir(item_path),
                        FileMode::Link => Item::Link(item_path),
                        FileMode::Commit => Item::Commit(item_path),
                    });
                }

                _ => {}
            }
        }

        Ok(paths)
    }

    /// Retrieves the items that were committed in the last commit.
    ///
    /// # Returns
    /// A `Result` containing a vector of `Item` or an error.
    pub fn get_post_commit_items(&self) -> Result<Vec<Item>> {
        info!("Querying post-commit items");
        let Ok(head) = self.repo.head() else {
            info!("Could not evaluate HEAD - assuming new repository.");
            return Ok(Vec::default());
        };

        let commit = self.repo.find_commit(head.target().unwrap())?;
        let parent = match commit.parent(0) {
            Ok(parent) => parent,
            Err(_) => {
                info!("Unable to query parent commit - assuming first commit");
                return self.get_list_of_all_files();
            }
        };

        let tree1 = commit.tree().ok();
        let tree2 = parent.tree().ok();

        let changes = self.repo.diff_tree_to_tree(
            tree2.as_ref(),
            tree1.as_ref(),
            Some(DiffOptions::new().ignore_submodules(false)),
        )?;

        let mut paths = Vec::new();
        for delta in changes.deltas() {
            let action = delta.status();
            match action {
                Delta::Deleted => continue,

                Delta::Added | Delta::Modified => {
                    let item_path = delta
                        .new_file()
                        .path()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();

                    paths.push(match delta.new_file().mode() {
                        FileMode::Unreadable => Item::Unknown(item_path),
                        FileMode::Blob | FileMode::BlobGroupWritable | FileMode::BlobExecutable => {
                            Item::File(item_path)
                        }
                        FileMode::Tree => Item::Dir(item_path),
                        FileMode::Link => Item::Link(item_path),
                        FileMode::Commit => Item::Commit(item_path),
                    });
                }

                _ => {}
            }
        }

        Ok(paths)
    }

    /// Retrieves a list of all files in the repository.
    ///
    /// # Returns
    /// A `Result` containing a vector of `Item` or an error.
    pub fn get_list_of_all_files(&self) -> Result<Vec<Item>> {
        let Ok(head) = self.repo.head() else {
            info!("Could not evaluate HEAD - assuming new repository.");
            return Ok(Vec::default());
        };

        let Ok(tree) = head.peel_to_tree() else {
            info!("Could not retrieve tree data. Likely new repository.");
            return Ok(Vec::default());
        };

        let mut paths = Vec::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |entry_path, entry| {
            let p = Path::new(entry_path);
            if let Some(name) = entry.name() {
                let p = p.join(name);
                p.to_str().map(|s| {
                    let s = s.to_owned();
                    paths.push(match entry.kind().unwrap_or(ObjectType::Any) {
                        ObjectType::Blob => Item::File(s),
                        ObjectType::Commit => Item::Commit(s),
                        ObjectType::Tree => Item::Dir(s),
                        ObjectType::Tag => Item::Commit(s),
                        ObjectType::Any => Item::Unknown(s),
                    });
                });
            }
            0
        })?;
        Ok(paths)
    }
}
