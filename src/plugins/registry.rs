use std::collections::HashMap;
use std::path::Path;

use super::loader::{load_plugins, load_plugins_from, LoadedPlugin};

#[derive(Debug, Clone)]
pub struct PluginRegistry {
    plugins: HashMap<String, LoadedPlugin>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn load_from(&mut self, plugins_dir: &Path) {
        let loaded = load_plugins(plugins_dir);
        for plugin in loaded {
            let name = plugin.manifest.name.clone();
            self.plugins.insert(name, plugin);
        }
    }

    pub fn load_from_multiple(&mut self, dirs: &[std::path::PathBuf]) {
        let loaded = load_plugins_from(dirs);
        for plugin in loaded {
            let name = plugin.manifest.name.clone();
            self.plugins.insert(name, plugin);
        }
    }

    pub fn reload(&mut self, dirs: &[std::path::PathBuf]) {
        self.plugins.clear();
        self.load_from_multiple(dirs);
    }

    pub fn get(&self, name: &str) -> Option<&LoadedPlugin> {
        self.plugins.get(name)
    }

    pub fn list(&self) -> Vec<&LoadedPlugin> {
        self.plugins.values().collect()
    }

    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    pub fn names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    pub fn remove(&mut self, name: &str) -> bool {
        self.plugins.remove(name).is_some()
    }

    pub fn clear(&mut self) {
        self.plugins.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_plugin(dir: &Path, name: &str) {
        let plugin_dir = dir.join(name);
        fs::create_dir_all(&plugin_dir).unwrap();
        let manifest = format!(
            r#"
name = "{name}"
version = "1.0.0"
description = "Plugin {name}"
command = "bash"
args = ["run.sh"]
"#
        );
        fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();
    }

    #[test]
    fn test_registry_empty() {
        let reg = PluginRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_registry_load() {
        let dir = std::env::temp_dir().join(format!("fe_reg_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        create_plugin(&dir, "test-plugin");

        let mut reg = PluginRegistry::new();
        reg.load_from(&dir);

        assert_eq!(reg.count(), 1);
        assert!(reg.contains("test-plugin"));
        assert_eq!(reg.names(), vec!["test-plugin"]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_registry_get() {
        let dir = std::env::temp_dir().join(format!("fe_reg_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        create_plugin(&dir, "my-plugin");

        let mut reg = PluginRegistry::new();
        reg.load_from(&dir);

        let plugin = reg.get("my-plugin").unwrap();
        assert_eq!(plugin.manifest.version, "1.0.0");

        assert!(reg.get("nonexistent").is_none());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_registry_list() {
        let dir = std::env::temp_dir().join(format!("fe_reg_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        create_plugin(&dir, "plugin-a");
        create_plugin(&dir, "plugin-b");

        let mut reg = PluginRegistry::new();
        reg.load_from(&dir);

        let list = reg.list();
        assert_eq!(list.len(), 2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_registry_remove() {
        let dir = std::env::temp_dir().join(format!("fe_reg_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        create_plugin(&dir, "to-remove");

        let mut reg = PluginRegistry::new();
        reg.load_from(&dir);

        assert!(reg.contains("to-remove"));
        assert!(reg.remove("to-remove"));
        assert!(!reg.contains("to-remove"));
        assert_eq!(reg.count(), 0);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_registry_remove_nonexistent() {
        let mut reg = PluginRegistry::new();
        assert!(!reg.remove("nonexistent"));
    }

    #[test]
    fn test_registry_clear() {
        let dir = std::env::temp_dir().join(format!("fe_reg_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        create_plugin(&dir, "plugin-a");
        create_plugin(&dir, "plugin-b");

        let mut reg = PluginRegistry::new();
        reg.load_from(&dir);
        assert_eq!(reg.count(), 2);

        reg.clear();
        assert_eq!(reg.count(), 0);
        assert!(reg.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_registry_reload() {
        let dir = std::env::temp_dir().join(format!("fe_reg_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        create_plugin(&dir, "plugin-a");

        let mut reg = PluginRegistry::new();
        reg.load_from(&dir);
        assert_eq!(reg.count(), 1);

        create_plugin(&dir, "plugin-b");
        reg.reload(&[dir.clone()]);
        assert_eq!(reg.count(), 2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_registry_load_from_multiple() {
        let dir1 = std::env::temp_dir().join(format!("fe_reg_test_a_{}", uuid::Uuid::new_v4()));
        let dir2 = std::env::temp_dir().join(format!("fe_reg_test_b_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir1).unwrap();
        fs::create_dir_all(&dir2).unwrap();
        create_plugin(&dir1, "from-a");
        create_plugin(&dir2, "from-b");

        let mut reg = PluginRegistry::new();
        reg.load_from_multiple(&[dir1, dir2]);

        assert_eq!(reg.count(), 2);
        assert!(reg.contains("from-a"));
        assert!(reg.contains("from-b"));
    }

    #[test]
    fn test_registry_duplicate_name_overwrites() {
        let dir = std::env::temp_dir().join(format!("fe_reg_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        create_plugin(&dir, "same-name");

        let mut reg = PluginRegistry::new();
        reg.load_from(&dir);
        assert_eq!(reg.count(), 1);
        assert_eq!(reg.get("same-name").unwrap().manifest.version, "1.0.0");

        // Load from same dir again — plugin already in registry, load adds again but HashMap overwrites key
        reg.load_from(&dir);
        assert_eq!(reg.count(), 1);

        fs::remove_dir_all(&dir).ok();
    }
}
