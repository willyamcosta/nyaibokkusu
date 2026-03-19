use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const TOOL_NAME: &str = "nyaibokkusu";

#[derive(Debug, Clone, Deserialize)]
pub struct Mount {
    pub path: String,
    #[serde(default)]
    pub rw: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub mounts: Vec<Mount>,
    #[serde(default)]
    pub exclude_mounts: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub gpu: bool,
    #[serde(default)]
    pub display: bool,
    #[serde(default)]
    pub docker: bool,
}

impl Config {
    pub fn default_base() -> Self {
        Config {
            mounts: vec![
                Mount {
                    path: "~/.claude".into(),
                    rw: true,
                },
                Mount {
                    path: "~/.claude.json".into(),
                    rw: true,
                },
                Mount {
                    path: "~/.aider".into(),
                    rw: true,
                },
                Mount {
                    path: "~/.aider.conf.yml".into(),
                    rw: true,
                },
                Mount {
                    path: "~/.codex".into(),
                    rw: true,
                },
                Mount {
                    path: "~/.config/opencode".into(),
                    rw: true,
                },
            ],
            ..Config::default()
        }
    }

    pub fn load_global(home: &str) -> Option<Self> {
        let path = global_config_path(home);
        Self::load_file(&path)
    }

    pub fn load_project(dir: &Path) -> Option<Self> {
        let path = dir.join(format!(".{TOOL_NAME}.toml"));
        Self::load_file(&path)
    }

    fn load_file(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        Some(toml::from_str(&content).unwrap_or_else(|e| {
            eprintln!("nyaibokkusu: error parsing {}: {e}", path.display());
            std::process::exit(1);
        }))
    }

    /// Merge base + global + project + CLI flags.
    /// Mounts/env concatenate, booleans OR, exclude_mounts filters prior mounts.
    pub fn merge(base: Self, global: Option<Self>, project: Option<Self>, cli: Self) -> Self {
        let mut mounts = base.mounts;
        let mut env = base.env;
        let mut command = base.command;
        let mut gpu = base.gpu;
        let mut display = base.display;
        let mut docker = base.docker;

        for layer in [global, project, Some(cli)] {
            let Some(layer) = layer else { continue };

            if !layer.command.is_empty() {
                command = layer.command;
            }

            if !layer.exclude_mounts.is_empty() {
                mounts.retain(|m| !layer.exclude_mounts.contains(&m.path));
            }

            mounts.extend(layer.mounts);
            env.extend(layer.env);

            gpu |= layer.gpu;
            display |= layer.display;
            docker |= layer.docker;
        }

        Config {
            command,
            mounts,
            exclude_mounts: Vec::new(),
            env,
            gpu,
            display,
            docker,
        }
    }

    pub fn expand_tilde(&mut self, home: &str) {
        for m in &mut self.mounts {
            if m.path.starts_with("~/") {
                m.path = format!("{}{}", home, &m.path[1..]);
            }
        }
        for p in &mut self.exclude_mounts {
            if p.starts_with("~/") {
                *p = format!("{}{}", home, &p[1..]);
            }
        }
    }
}

fn global_config_path(home: &str) -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    PathBuf::from(base).join(format!("{TOOL_NAME}.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mount(path: &str, rw: bool) -> Mount {
        Mount {
            path: path.into(),
            rw,
        }
    }

    fn base_config(command: &str, mounts: Vec<Mount>) -> Config {
        Config {
            command: command.into(),
            mounts,
            ..Config::default()
        }
    }

    #[test]
    fn merge_no_config() {
        let merged = Config::merge(Config::default(), None, None, Config::default());
        assert!(merged.command.is_empty());
        assert!(merged.mounts.is_empty());
        assert!(!merged.gpu);
    }

    #[test]
    fn merge_dotfile_overrides_command() {
        let p = base_config("claude", vec![]);
        let d = Config {
            command: "my-tool".into(),
            ..Config::default()
        };
        let merged = Config::merge(p, None, Some(d), Config::default());
        assert_eq!(merged.command, "my-tool");
    }

    #[test]
    fn merge_exclude_mounts() {
        let base = base_config(
            "claude",
            vec![mount("~/.claude", true), mount("~/.claude.json", true)],
        );
        let d = Config {
            exclude_mounts: vec!["~/.claude.json".into()],
            ..Config::default()
        };
        let merged = Config::merge(base, Some(d), None, Config::default());
        assert_eq!(merged.mounts.len(), 1);
        assert_eq!(merged.mounts[0].path, "~/.claude");
    }

    #[test]
    fn merge_mounts_concatenate() {
        let base = base_config("claude", vec![mount("~/.claude", true)]);
        let d = Config {
            mounts: vec![mount("~/.extra", false)],
            ..Config::default()
        };
        let cli = Config {
            mounts: vec![mount("/tmp/foo", false)],
            ..Config::default()
        };
        let merged = Config::merge(base, Some(d), None, cli);
        assert_eq!(merged.mounts.len(), 3);
    }

    #[test]
    fn merge_booleans_or() {
        let base = Config {
            gpu: true,
            ..Config::default()
        };
        let d = Config {
            docker: true,
            ..Config::default()
        };
        let cli = Config {
            display: true,
            ..Config::default()
        };
        let merged = Config::merge(base, Some(d), None, cli);
        assert!(merged.gpu);
        assert!(merged.docker);
        assert!(merged.display);
    }

    #[test]
    fn merge_env_concatenates() {
        let base = Config {
            env: HashMap::from([("A".into(), "1".into())]),
            ..Config::default()
        };
        let d = Config {
            env: HashMap::from([("B".into(), "2".into())]),
            ..Config::default()
        };
        let merged = Config::merge(base, Some(d), None, Config::default());
        assert_eq!(merged.env.get("A").unwrap(), "1");
        assert_eq!(merged.env.get("B").unwrap(), "2");
    }

    #[test]
    fn merge_env_project_overrides_base() {
        let base = Config {
            env: HashMap::from([("A".into(), "old".into())]),
            ..Config::default()
        };
        let d = Config {
            env: HashMap::from([("A".into(), "new".into())]),
            ..Config::default()
        };
        let merged = Config::merge(base, Some(d), None, Config::default());
        assert_eq!(merged.env.get("A").unwrap(), "new");
    }

    #[test]
    fn expand_tilde_replaces_home() {
        let mut c = Config {
            mounts: vec![mount("~/.claude", true), mount("/abs/path", false)],
            exclude_mounts: vec!["~/.claude.json".into()],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "/home/user/.claude");
        assert_eq!(c.mounts[1].path, "/abs/path");
        assert_eq!(c.exclude_mounts[0], "/home/user/.claude.json");
    }

    #[test]
    fn parse_toml_dotfile() {
        let toml_str = r#"
command = "my-tool"
gpu = true
exclude_mounts = ["~/.claude.json"]

[[mounts]]
path = "~/.extra"
rw = false

[env]
MY_VAR = "value"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.command, "my-tool");
        assert!(config.gpu);
        assert_eq!(config.exclude_mounts, vec!["~/.claude.json"]);
        assert_eq!(config.mounts.len(), 1);
        assert_eq!(config.mounts[0].path, "~/.extra");
        assert!(!config.mounts[0].rw);
        assert_eq!(config.env.get("MY_VAR").unwrap(), "value");
    }
}
