use std::path::Path;
use super::manifest::PluginManifest;

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub dir: std::path::PathBuf,
    pub manifest: PluginManifest,
}

pub fn load_plugins(plugins_dir: &Path) -> Vec<LoadedPlugin> {
    if !plugins_dir.exists() {
        tracing::debug!("Plugins directory does not exist: {:?}", plugins_dir);
        return Vec::new();
    }

    let mut plugins = Vec::new();

    let entries = match std::fs::read_dir(plugins_dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("Failed to read plugins directory {:?}: {}", plugins_dir, e);
            return Vec::new();
        }
    };

    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }

        let manifest_path = plugin_dir.join("plugin.toml");
        if !manifest_path.exists() {
            tracing::debug!("Skipping {:?}: no plugin.toml found", plugin_dir);
            continue;
        }

        match PluginManifest::from_path(&manifest_path) {
            Ok(manifest) => {
                let name = manifest.name.clone();
                plugins.push(LoadedPlugin {
                    dir: plugin_dir,
                    manifest,
                });
                tracing::info!("Loaded plugin '{}'", name);
            }
            Err(e) => {
                tracing::warn!("Failed to load plugin from {:?}: {}", manifest_path, e);
            }
        }
    }

    plugins
}

pub fn load_plugins_from(paths: &[std::path::PathBuf]) -> Vec<LoadedPlugin> {
    let mut all = Vec::new();
    for dir in paths {
        all.extend(load_plugins(dir));
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_plugin(dir: &Path, name: &str) {
        let plugin_dir = dir.join(name);
        fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = format!(
            r#"
name = "{name}"
version = "1.0.0"
description = "Plugin {name}"
command = "bash"
args = ["run.sh"]

[[tools]]
name = "echo"
description = "Echo tool"
params = {{ type = "object", properties = {{ msg = {{ type = "string" }} }}, required = ["msg"] }}
"#
        );
        fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();
    }

    #[test]
    fn test_load_plugins_empty_dir() {
        let dir = std::env::temp_dir().join(format!("fe_plugins_test_{}", uuid::Uuid::new_v4()));
        let plugins = load_plugins(&dir);
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_load_plugins_nonexistent_dir() {
        let plugins = load_plugins(Path::new("/tmp/nonexistent_plugins_dir_xyz"));
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_load_single_plugin() {
        let dir = std::env::temp_dir().join(format!("fe_plugins_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        create_test_plugin(&dir, "test-plugin");

        let plugins = load_plugins(&dir);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest.name, "test-plugin");
        assert_eq!(plugins[0].manifest.tools.len(), 1);
        assert_eq!(plugins[0].manifest.tools[0].name, "echo");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_multiple_plugins() {
        let dir = std::env::temp_dir().join(format!("fe_plugins_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        create_test_plugin(&dir, "plugin-a");
        create_test_plugin(&dir, "plugin-b");

        let plugins = load_plugins(&dir);
        assert_eq!(plugins.len(), 2);

        let names: Vec<&str> = plugins.iter().map(|p| p.manifest.name.as_str()).collect();
        assert!(names.contains(&"plugin-a"));
        assert!(names.contains(&"plugin-b"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_skip_dirs_without_manifest() {
        let dir = std::env::temp_dir().join(format!("fe_plugins_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(dir.join("empty-dir")).unwrap();
        create_test_plugin(&dir, "real-plugin");

        let plugins = load_plugins(&dir);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest.name, "real-plugin");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_skip_files_not_dirs() {
        let dir = std::env::temp_dir().join(format!("fe_plugins_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("not-a-plugin"), "some content").unwrap();
        create_test_plugin(&dir, "valid-plugin");

        let plugins = load_plugins(&dir);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest.name, "valid-plugin");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_load_plugins_from_multiple_paths() {
        let dir1 = std::env::temp_dir().join(format!("fe_plugins_test_a_{}", uuid::Uuid::new_v4()));
        let dir2 = std::env::temp_dir().join(format!("fe_plugins_test_b_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir1).unwrap();
        fs::create_dir_all(&dir2).unwrap();
        create_test_plugin(&dir1, "plugin-from-a");
        create_test_plugin(&dir2, "plugin-from-b");

        let plugins = load_plugins_from(&[dir1.clone(), dir2.clone()]);
        assert_eq!(plugins.len(), 2);

        let names: Vec<&str> = plugins.iter().map(|p| p.manifest.name.as_str()).collect();
        assert!(names.contains(&"plugin-from-a"));
        assert!(names.contains(&"plugin-from-b"));

        fs::remove_dir_all(&dir1).ok();
        fs::remove_dir_all(&dir2).ok();
    }
}
