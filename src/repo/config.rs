use anyhow::Result;
use git2::{Config, Repository};
use std::cell::RefCell;
use std::rc::Rc;

/// This trait allows creating git config configuration sections associated with
/// category_id and action_id parameters, such that each entry looks like
/// <category_id>.<action_id>.<key> = <val>.
#[cfg_attr(test, mockall::automock)]
pub trait GitConfigManager {
    /// Returns a GitConfig instance for the given category and action.
    ///
    /// # Arguments
    ///
    /// * `category_id` - A string slice that holds the category identifier
    /// * `action_id` - A string slice that holds the action identifier
    ///
    /// # Returns
    ///
    /// A boxed trait object implementing GitConfig
    fn get_config_for(&self, category_id: &str, action_id: &str) -> Box<dyn GitConfig>;
}

/// This trait persists data in git config file appropriate for the current git workdir.
#[cfg_attr(test, mockall::automock)]
pub trait GitConfig {
    /// Retrieves the value for the given key from the git config, or returns the default value if not found.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up in the git config.
    /// * `default` - The default value to return if the key is not found.
    ///
    /// # Returns
    ///
    /// The value associated with the key, or the default value if not found.
    fn get_or_default(&self, key: &str, default: &str) -> String;

    /// Sets the value for the given key in the git config.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to set in the git config.
    /// * `value` - The value to associate with the key.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure of the operation.
    fn set(&mut self, key: &str, value: &str) -> Result<()>;

    /// Removes the given key from the git config.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to remove from the git config.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure of the operation.
    fn remove(&mut self, key: &str) -> Result<()>;
}

/// Trait for abstracting configuration storage operations.
///
/// This trait provides a common interface for interacting with configuration
/// storage, allowing for different implementations (e.g., file-based, in-memory,
/// or database-backed storage) to be used interchangeably.
#[cfg_attr(test, mockall::automock)]
trait ConfigTrait {
    /// Checks if a key exists in the configuration.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to check for existence.
    ///
    /// # Returns
    ///
    /// `true` if the key exists, `false` otherwise.
    fn has_key(&self, key: &str) -> bool;

    /// Retrieves the value associated with a key from the configuration.
    ///
    /// # Arguments
    ///
    /// * `key` - The key whose value to retrieve.
    ///
    /// # Returns
    ///
    /// A `Result` containing the value as a `String` if successful,
    /// or an error if the key doesn't exist or there's a problem retrieving the value.
    fn get_key(&self, key: &str) -> Result<String>;

    /// Sets a key-value pair in the configuration.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to set.
    /// * `val` - The value to associate with the key.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure of the operation.
    fn set_key(&mut self, key: &str, val: &str) -> Result<()>;

    /// Removes a key-value pair from the configuration.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to remove.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure of the operation.
    fn remove_key(&mut self, key: &str) -> Result<()>;
}

impl ConfigTrait for Config {
    fn has_key(&self, key: &str) -> bool {
        self.get_entry(key).is_ok()
    }

    fn get_key(&self, key: &str) -> Result<String> {
        Ok(self.get_string(key)?)
    }

    fn set_key(&mut self, key: &str, val: &str) -> Result<()> {
        Ok(self.set_str(key, val)?)
    }

    fn remove_key(&mut self, key: &str) -> Result<()> {
        Ok(self.remove(key)?)
    }
}

pub struct GitConfigManagerImpl {
    config: Rc<RefCell<dyn ConfigTrait>>,
}

struct GitConfigImpl {
    config: Rc<RefCell<dyn ConfigTrait>>,
    section: String,
    hook: String,
}

impl GitConfigManagerImpl {
    fn from_config<T: ConfigTrait + 'static>(config: T) -> Box<dyn GitConfigManager> {
        Box::new(GitConfigManagerImpl {
            config: Rc::new(RefCell::new(config)),
        })
    }

    pub fn new(repo: &Rc<Repository>) -> Result<Box<dyn GitConfigManager>> {
        Ok(Self::from_config(repo.config()?))
    }
}

impl GitConfigManager for GitConfigManagerImpl {
    fn get_config_for(&self, category_id: &str, hook_id: &str) -> Box<dyn GitConfig> {
        Box::new(GitConfigImpl {
            config: self.config.clone(),
            section: category_id.to_string(),
            hook: hook_id.to_string(),
        })
    }
}

impl GitConfig for GitConfigImpl {
    fn get_or_default(&self, key: &str, default: &str) -> String {
        let full_key = format!("{}.{}.{}", self.section, self.hook, key);
        self.config
            .borrow()
            .get_key(&full_key)
            .unwrap_or(default.to_string())
    }

    fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let full_key = format!("{}.{}.{}", self.section, self.hook, key);
        self.config.borrow_mut().set_key(&full_key, value)?;
        Ok(())
    }

    fn remove(&mut self, key: &str) -> Result<()> {
        let full_key = format!("{}.{}.{}", self.section, self.hook, key);
        if self.config.borrow().has_key(&full_key) {
            self.config.borrow_mut().remove_key(&full_key)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::bail;
    use mockall::predicate::*;

    #[test]
    fn test_get_or_default() {
        let mut mock_config = MockConfigTrait::new();

        mock_config
            .expect_get_key()
            .with(eq("test.hook.key"))
            .returning(|_| Ok("value".into()));

        let config_manager = GitConfigManagerImpl::from_config(mock_config);
        let git_config = config_manager.get_config_for("test", "hook");

        assert_eq!(git_config.get_or_default("key", "default"), "value");
    }

    #[test]
    fn test_get_or_default_with_default() {
        let mut mock_config = MockConfigTrait::new();
        mock_config
            .expect_get_key()
            .with(eq("test.hook.key"))
            .returning(|_| bail!("Test key not found"));

        let config_manager = GitConfigManagerImpl::from_config(mock_config);
        let git_config = config_manager.get_config_for("test", "hook");

        assert_eq!(git_config.get_or_default("key", "default"), "default");
    }

    #[test]
    fn test_set() {
        let mut mock_config = MockConfigTrait::new();
        mock_config
            .expect_set_key()
            .with(eq("test.hook.key"), eq("value"))
            .returning(|_, _| Ok(()));

        let config_manager = GitConfigManagerImpl::from_config(mock_config);
        let mut git_config = config_manager.get_config_for("test", "hook");

        assert!(git_config.set("key", "value").is_ok());
    }

    #[test]
    fn test_remove_existing() {
        let mut mock_config = MockConfigTrait::new();
        mock_config
            .expect_has_key()
            .with(eq("test.hook.key"))
            .return_const(true);
        mock_config
            .expect_remove_key()
            .with(eq("test.hook.key"))
            .returning(|_| Ok(()));

        let config_manager = GitConfigManagerImpl::from_config(mock_config);
        let mut git_config = config_manager.get_config_for("test", "hook");

        assert!(git_config.remove("key").is_ok());
    }

    #[test]
    fn test_remove_non_existing() {
        let mut mock_config = MockConfigTrait::new();
        mock_config
            .expect_has_key()
            .with(eq("test.hook.key"))
            .return_const(false);

        mock_config.expect_remove_key().never();

        let config_manager = GitConfigManagerImpl::from_config(mock_config);
        let mut git_config = config_manager.get_config_for("test", "hook");

        assert!(git_config.remove("key").is_ok());
    }
}
