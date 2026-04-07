use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const TOOL_NAME: &str = "nyaibokkusu";

#[derive(Debug, Clone, Deserialize)]
pub struct Mount {
    pub path: String,
    #[serde(default)]
    pub dest: Option<String>,
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
                    dest: None,
                    rw: true,
                },
                Mount {
                    path: "~/.claude.json".into(),
                    dest: None,
                    rw: true,
                },
                Mount {
                    path: "~/.aider".into(),
                    dest: None,
                    rw: true,
                },
                Mount {
                    path: "~/.aider.conf.yml".into(),
                    dest: None,
                    rw: true,
                },
                Mount {
                    path: "~/.codex".into(),
                    dest: None,
                    rw: true,
                },
                Mount {
                    path: "${XDG_CONFIG_HOME:-~/.config}/opencode".into(),
                    dest: None,
                    rw: true,
                },
                Mount {
                    path: "${XDG_CACHE_HOME:-~/.cache}/opencode".into(),
                    dest: None,
                    rw: true,
                },
                Mount {
                    path: "${XDG_DATA_HOME:-~/.local/share}/opencode".into(),
                    dest: None,
                    rw: true,
                },
                Mount {
                    path: "${XDG_STATE_HOME:-~/.local/state}/opencode".into(),
                    dest: None,
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
            m.path = expand_env_vars(&m.path, home);
            if let Some(ref mut dest) = m.dest {
                *dest = expand_env_vars(dest, home);
            }
        }
        for p in &mut self.exclude_mounts {
            *p = expand_env_vars(p, home);
        }
        for v in self.env.values_mut() {
            *v = expand_env_vars(v, home);
        }
    }
}

fn expand_env_vars(s: &str, home: &str) -> String {
    let s = if s.starts_with("~/") {
        format!("{}{}", home, &s[1..])
    } else {
        s.to_string()
    };

    let mut output = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'$' || i + 1 >= bytes.len() {
            output.push(bytes[i] as char);
            i += 1;
            continue;
        }

        if bytes[i + 1] == b'{' {
            i += 2;
            let start = i;
            while i < bytes.len() && bytes[i] != b'}' {
                i += 1;
            }
            if i >= bytes.len() {
                output.push_str("${");
                i = start;
                continue;
            }
            output.push_str(&expand_braced_var(&s[start..i], home));
            i += 1;
        } else if bytes[i + 1].is_ascii_alphanumeric() || bytes[i + 1] == b'_' {
            i += 1;
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            output.push_str(&std::env::var(&s[start..i]).unwrap_or_default());
        } else {
            output.push('$');
            i += 1;
        }
    }
    output
}

fn expand_braced_var(var_part: &str, home: &str) -> String {
    if var_part.is_empty() {
        return "${}".into();
    }

    let Some(colon) = var_part.find(":-") else {
        return if var_part.chars().all(|c| c.is_alphanumeric() || c == '_') {
            std::env::var(var_part).unwrap_or_default()
        } else {
            format!("${{{}}}", var_part)
        };
    };

    let (var_name, default) = var_part.split_at(colon);
    let default = &default[2..];

    if var_name.is_empty() || !var_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return format!("${{{}}}", var_part);
    }

    std::env::var(var_name).unwrap_or_else(|_| {
        if default.starts_with("~/") {
            format!("{}{}", home, &default[1..])
        } else {
            default.into()
        }
    })
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
            dest: None,
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

    #[test]
    fn expand_tilde_replaces_dest() {
        let mut c = Config {
            mounts: vec![Mount {
                path: "/etc/hosts".into(),
                dest: Some("~/.config/hosts".into()),
                rw: false,
            }],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "/etc/hosts");
        assert_eq!(c.mounts[0].dest, Some("/home/user/.config/hosts".into()));
    }

    #[test]
    fn parse_toml_with_dest() {
        let toml_str = r#"
[[mounts]]
path = "/custom/src"
dest = "~/.config/opencode"
rw = true
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.mounts.len(), 1);
        assert_eq!(config.mounts[0].path, "/custom/src");
        assert_eq!(config.mounts[0].dest, Some("~/.config/opencode".into()));
        assert!(config.mounts[0].rw);
    }

    #[test]
    fn expand_env_var_simple() {
        std::env::set_var("TEST_VAR", "/test/path");
        let mut c = Config {
            mounts: vec![mount("$TEST_VAR/something", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "/test/path/something");
        std::env::remove_var("TEST_VAR");
    }

    #[test]
    fn expand_env_var_braced() {
        std::env::set_var("MY_VAR", "/my/path");
        let mut c = Config {
            mounts: vec![mount("${MY_VAR}/sub", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "/my/path/sub");
        std::env::remove_var("MY_VAR");
    }

    #[test]
    fn expand_env_var_with_default() {
        std::env::remove_var("UNSET_VAR");
        let mut c = Config {
            mounts: vec![mount("${UNSET_VAR:-/default}/path", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "/default/path");
    }

    #[test]
    fn expand_env_var_with_tilde_default() {
        std::env::remove_var("UNSET_XDG");
        let mut c = Config {
            mounts: vec![mount("${UNSET_XDG:-~/.config}/app", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "/home/user/.config/app");
    }

    #[test]
    fn expand_env_var_overrides_default() {
        std::env::set_var("SET_VAR", "/overridden");
        let mut c = Config {
            mounts: vec![mount("${SET_VAR:-/default}/path", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "/overridden/path");
        std::env::remove_var("SET_VAR");
    }

    #[test]
    fn expand_multiple_env_vars() {
        std::env::set_var("TEST_A", "/path_a");
        std::env::set_var("TEST_B", "path_b");
        let mut c = Config {
            mounts: vec![mount("$TEST_A/$TEST_B/file", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "/path_a/path_b/file");
        std::env::remove_var("TEST_A");
        std::env::remove_var("TEST_B");
    }

    #[test]
    fn expand_env_var_empty_value() {
        std::env::set_var("EMPTY_VAR_001", "");
        let mut c = Config {
            mounts: vec![mount("${EMPTY_VAR_001}/path", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "/path");
        std::env::remove_var("EMPTY_VAR_001");
    }

    #[test]
    fn expand_tilde_only_at_start() {
        let mut c = Config {
            mounts: vec![mount("/path/~/file", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        // ~ in middle of path should NOT be expanded
        assert_eq!(c.mounts[0].path, "/path/~/file");
    }

    #[test]
    fn expand_exclude_mounts_env_vars() {
        std::env::set_var("EXCL_VAR_001", "/tmp/exclude");
        let mut c = Config {
            exclude_mounts: vec!["$EXCL_VAR_001/dir".into()],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.exclude_mounts[0], "/tmp/exclude/dir");
        std::env::remove_var("EXCL_VAR_001");
    }

    #[test]
    fn expand_env_var_malformed_empty() {
        // Empty ${} should not expand (leaves as-is or breaks out)
        let mut c = Config {
            mounts: vec![mount("${}/path", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "${}/path");
    }

    #[test]
    fn expand_env_var_malformed_unclosed() {
        // Unclosed ${VAR should not expand
        let mut c = Config {
            mounts: vec![mount("${UNCLOSED/path", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "${UNCLOSED/path");
    }

    #[test]
    fn expand_env_var_invalid_name() {
        // Invalid characters in var name should not expand
        let mut c = Config {
            mounts: vec![mount("${INVALID-NAME}/path", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.mounts[0].path, "${INVALID-NAME}/path");
    }

    #[test]
    fn expand_env_var_no_recursive_in_value() {
        // Values containing $VAR should NOT be expanded (no recursive expansion)
        std::env::set_var("OUTER", "$INNER/path");
        std::env::set_var("INNER", "/should/not/expand");
        let mut c = Config {
            mounts: vec![mount("$OUTER/file", true)],
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        // OUTER is expanded literally - the $INNER in its value is NOT expanded again
        assert_eq!(c.mounts[0].path, "$INNER/path/file");
        std::env::remove_var("OUTER");
        std::env::remove_var("INNER");
    }

    #[test]
    fn expand_env_values_in_config() {
        std::env::set_var("MY_HOME", "/custom/home");
        let mut c = Config {
            env: [("PATH_PREFIX".into(), "$MY_HOME/bin".into())].into(),
            ..Config::default()
        };
        c.expand_tilde("/home/user");
        assert_eq!(c.env.get("PATH_PREFIX").unwrap(), "/custom/home/bin");
        std::env::remove_var("MY_HOME");
    }
}
