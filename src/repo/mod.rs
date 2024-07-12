mod config;
mod repo;

pub use config::GitConfig;
pub use config::GitConfigManager;
pub use config::GitConfigManagerImpl;
pub use repo::GitRepo;

#[cfg(test)]
pub mod test {
    pub use super::config::MockGitConfig;
    pub use super::config::MockGitConfigManager;
}
