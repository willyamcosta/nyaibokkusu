use crate::config::Config;
use std::path::Path;

pub fn build_args(config: &Config, home: &str, project_dir: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    // Core filesystem
    push(&mut args, &["--dev", "/dev"]);
    push(&mut args, &["--proc", "/proc"]);
    push(&mut args, &["--tmpfs", "/tmp"]);
    push(&mut args, &["--ro-bind", "/nix/store", "/nix/store"]);
    push(
        &mut args,
        &["--ro-bind", "/run/current-system", "/run/current-system"],
    );
    push(&mut args, &["--ro-bind", "/etc/profiles", "/etc/profiles"]);
    push(&mut args, &["--ro-bind", "/etc/static", "/etc/static"]);
    push(
        &mut args,
        &["--ro-bind", "/etc/resolv.conf", "/etc/resolv.conf"],
    );
    push(&mut args, &["--ro-bind", "/etc/hosts", "/etc/hosts"]);
    push(&mut args, &["--ro-bind", "/etc/ssl", "/etc/ssl"]);
    push(&mut args, &["--ro-bind", "/etc/nix", "/etc/nix"]);
    push(&mut args, &["--ro-bind", "/etc/passwd", "/etc/passwd"]);
    push(&mut args, &["--ro-bind", "/etc/group", "/etc/group"]);
    push(
        &mut args,
        &["--ro-bind", "/etc/nsswitch.conf", "/etc/nsswitch.conf"],
    );
    push(
        &mut args,
        &["--symlink", "/run/current-system/sw/bin", "/bin"],
    );
    push(
        &mut args,
        &["--symlink", "/run/current-system/sw/bin", "/usr/bin"],
    );

    // Home as tmpfs, then project dir rw
    push(&mut args, &["--tmpfs", home]);

    // Nix profile mounts
    let nix_profile = format!("{home}/.nix-profile");
    if Path::new(&nix_profile).is_dir() {
        push(&mut args, &["--ro-bind", &nix_profile, &nix_profile]);
    }
    let nix_state = format!("{home}/.local/state/nix");
    if Path::new(&nix_state).is_dir() {
        push(&mut args, &["--ro-bind", &nix_state, &nix_state]);
    }

    // Project directory (rw)
    push(&mut args, &["--bind", project_dir, project_dir]);
    push(&mut args, &["--chdir", project_dir]);

    // Namespaces
    push(
        &mut args,
        &["--unshare-pid", "--unshare-uts", "--unshare-ipc"],
    );
    push(&mut args, &["--die-with-parent"]);
    push(&mut args, &["--hostname", "nyaibokkusu"]);

    // Environment
    push(&mut args, &["--clearenv"]);
    let path_val = format!(
        "/run/current-system/sw/bin:/nix/var/nix/profiles/default/bin:/etc/profiles/per-user/{user}/bin:{home}/.nix-profile/bin",
        user = std::env::var("USER").unwrap_or_else(|_| "nobody".into())
    );
    push(&mut args, &["--setenv", "PATH", &path_val]);
    push(&mut args, &["--setenv", "HOME", home]);
    setenv_from(&mut args, "TERM", "xterm-256color");
    setenv_from(&mut args, "LANG", "en_US.UTF-8");
    setenv_or(&mut args, "USER", "nobody");
    setenv_from(&mut args, "SHELL", "/bin/bash");
    setenv_or(&mut args, "NIX_PATH", "");
    setenv_or(&mut args, "NIX_PROFILES", "/run/current-system/sw");
    setenv_from(
        &mut args,
        "LOCALE_ARCHIVE",
        "/run/current-system/sw/lib/locale/locale-archive",
    );
    setenv_from(&mut args, "TZDIR", "/etc/zoneinfo");
    setenv_from(
        &mut args,
        "SSL_CERT_FILE",
        "/etc/ssl/certs/ca-certificates.crt",
    );
    push(&mut args, &["--setenv", "PS1", "(nyaibokkusu) \\w \\$ "]);

    // Config mounts (filtered to existing paths)
    for m in &config.mounts {
        let src = &m.path;
        let exists = Path::new(src).exists() || std::fs::symlink_metadata(src).is_ok();
        if !exists {
            continue;
        }
        if m.rw {
            push(&mut args, &["--bind", src, src]);
        } else {
            push(&mut args, &["--ro-bind", src, src]);
        }
    }

    // Config env
    let mut env_keys: Vec<&String> = config.env.keys().collect();
    env_keys.sort();
    for k in env_keys {
        push(&mut args, &["--setenv", k, &config.env[k]]);
    }

    // GPU
    if config.gpu {
        if Path::new("/dev/dri").is_dir() {
            push(&mut args, &["--dev-bind", "/dev/dri", "/dev/dri"]);
        }
        // Scan /dev/nvidia*
        if let Ok(entries) = std::fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("nvidia") {
                    let p = entry.path();
                    let p = p.to_string_lossy();
                    push(&mut args, &["--dev-bind", &p, &p]);
                }
            }
        }
        if Path::new("/run/opengl-driver").is_dir() {
            push(
                &mut args,
                &["--ro-bind", "/run/opengl-driver", "/run/opengl-driver"],
            );
        }
    }

    // Display
    if config.display {
        if let Ok(display) = std::env::var("DISPLAY") {
            let xsock = "/tmp/.X11-unix";
            if Path::new(xsock).is_dir() {
                push(&mut args, &["--ro-bind", xsock, xsock]);
            }
            push(&mut args, &["--setenv", "DISPLAY", &display]);
        }
        if let Ok(wayland) = std::env::var("WAYLAND_DISPLAY") {
            let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| {
                // Read real UID from /proc/self/status, fall back to PID
                let uid = std::process::id();
                let uid = read_uid().unwrap_or(uid);
                format!("/run/user/{uid}")
            });
            let wsock = format!("{xdg}/{wayland}");
            if unix_socket_exists(&wsock) {
                push(&mut args, &["--ro-bind", &wsock, &wsock]);
            }
            push(&mut args, &["--setenv", "WAYLAND_DISPLAY", &wayland]);
            push(&mut args, &["--setenv", "XDG_RUNTIME_DIR", &xdg]);
        }
    }

    // Nix daemon
    let daemon_sock = "/nix/var/nix/daemon-socket";
    let has_daemon = Path::new(daemon_sock).exists();
    if has_daemon {
        push(&mut args, &["--bind", daemon_sock, daemon_sock]);
    }
    let nix_profiles = "/nix/var/nix/profiles";
    if Path::new(nix_profiles).is_dir() {
        push(&mut args, &["--ro-bind", nix_profiles, nix_profiles]);
    }
    if has_daemon {
        push(&mut args, &["--setenv", "NIX_REMOTE", "daemon"]);
    }

    // Docker
    if config.docker {
        let sock = "/var/run/docker.sock";
        if unix_socket_exists(sock) {
            push(&mut args, &["--bind", sock, sock]);
        }
    }

    args
}

fn push(args: &mut Vec<String>, items: &[&str]) {
    for item in items {
        args.push((*item).to_string());
    }
}

fn setenv_from(args: &mut Vec<String>, var: &str, default: &str) {
    let val = std::env::var(var).unwrap_or_else(|_| default.to_string());
    push(args, &["--setenv", var, &val]);
}

fn setenv_or(args: &mut Vec<String>, var: &str, default: &str) {
    let val = std::env::var(var).unwrap_or_else(|_| default.to_string());
    push(args, &["--setenv", var, &val]);
}

fn unix_socket_exists(path: &str) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

fn read_uid() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Mount};

    fn empty_config() -> Config {
        Config::default()
    }

    #[test]
    fn base_args_contain_core_mounts() {
        let args = build_args(&empty_config(), "/home/test", "/tmp/project");
        assert!(args.contains(&"--dev".to_string()));
        assert!(args.contains(&"--proc".to_string()));
        assert!(args.contains(&"/nix/store".to_string()));
        assert!(args.contains(&"--clearenv".to_string()));
        assert!(args.contains(&"--die-with-parent".to_string()));
        assert!(args.contains(&"--hostname".to_string()));
        assert!(args.contains(&"nyaibokkusu".to_string()));
    }

    #[test]
    fn project_dir_is_rw_bound() {
        let args = build_args(&empty_config(), "/home/test", "/tmp/project");
        // Find --bind /tmp/project /tmp/project
        let bind_idx = args
            .windows(3)
            .position(|w| w[0] == "--bind" && w[1] == "/tmp/project" && w[2] == "/tmp/project");
        assert!(bind_idx.is_some());
    }

    #[test]
    fn home_is_tmpfs() {
        let args = build_args(&empty_config(), "/home/test", "/tmp/project");
        let idx = args
            .windows(2)
            .position(|w| w[0] == "--tmpfs" && w[1] == "/home/test");
        assert!(idx.is_some());
    }

    #[test]
    fn chdir_to_project() {
        let args = build_args(&empty_config(), "/home/test", "/tmp/project");
        let idx = args
            .windows(2)
            .position(|w| w[0] == "--chdir" && w[1] == "/tmp/project");
        assert!(idx.is_some());
    }

    #[test]
    fn env_vars_from_config() {
        let config = Config {
            env: [("FOO".into(), "bar".into())].into(),
            ..Config::default()
        };
        let args = build_args(&config, "/home/test", "/tmp/project");
        let idx = args
            .windows(3)
            .position(|w| w[0] == "--setenv" && w[1] == "FOO" && w[2] == "bar");
        assert!(idx.is_some());
    }

    #[test]
    fn path_includes_user_profile_bin() {
        let original = std::env::var("USER").ok();
        std::env::set_var("USER", "alice");
        let args = build_args(&empty_config(), "/home/test", "/tmp/project");
        let idx = args.windows(3).position(|w| {
            w[0] == "--setenv"
                && w[1] == "PATH"
                && w[2].contains("/etc/profiles/per-user/alice/bin")
        });
        assert!(idx.is_some());
        match original {
            Some(value) => std::env::set_var("USER", value),
            None => std::env::remove_var("USER"),
        }
    }

    #[test]
    fn nix_remote_set_when_daemon_available() {
        let args = build_args(&empty_config(), "/home/test", "/tmp/project");
        if Path::new("/nix/var/nix/daemon-socket").exists() {
            let idx = args
                .windows(3)
                .position(|w| w[0] == "--setenv" && w[1] == "NIX_REMOTE" && w[2] == "daemon");
            assert!(idx.is_some());
        }
    }

    #[test]
    fn nonexistent_mount_skipped() {
        let config = Config {
            mounts: vec![Mount {
                path: "/nonexistent/path/that/does/not/exist".into(),
                rw: false,
            }],
            ..Config::default()
        };
        let args = build_args(&config, "/home/test", "/tmp/project");
        assert!(!args.contains(&"/nonexistent/path/that/does/not/exist".to_string()));
    }

    #[test]
    fn existing_mount_included() {
        // /tmp always exists
        let config = Config {
            mounts: vec![Mount {
                path: "/etc/hosts".into(),
                rw: false,
            }],
            ..Config::default()
        };
        let args = build_args(&config, "/home/test", "/tmp/project");
        // Should appear as --ro-bind /etc/hosts /etc/hosts from config mounts
        let count = args
            .windows(3)
            .filter(|w| w[0] == "--ro-bind" && w[1] == "/etc/hosts" && w[2] == "/etc/hosts")
            .count();
        // At least 2: one from base mounts, one from config mounts
        assert!(count >= 2);
    }
}
