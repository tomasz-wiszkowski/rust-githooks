mod config;
mod item;
mod repo;

pub use config::GitConfig;
pub use config::GitConfigManager;
pub use item::Item;
pub use repo::GitRepo;

use config::GitConfigManagerImpl;

#[cfg(test)]
pub mod test {
    pub use super::config::MockGitConfig;
    pub use super::config::MockGitConfigManager;
}
