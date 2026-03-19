mod bwrap;
mod config;

use config::Config;
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::process::Command;

fn main() {
    let mut cli = Config::default();
    let mut command_override: Vec<String> = Vec::new();
    let mut no_global_config = false;

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next().unwrap_or_else(|e| {
        eprintln!("nyaibokkusu: {e}");
        std::process::exit(1);
    }) {
        use lexopt::Arg::*;
        match arg {
            Long("map") => {
                let path = value_string(&mut parser, "--map");
                cli.mounts.push(config::Mount { path, rw: false });
            }
            Long("rw-map") => {
                let path = value_string(&mut parser, "--rw-map");
                cli.mounts.push(config::Mount { path, rw: true });
            }
            Long("gpu") => cli.gpu = true,
            Long("display") => cli.display = true,
            Long("docker") => cli.docker = true,
            Long("no-global-config") => no_global_config = true,
            Short('h') | Long("help") => {
                print_help();
                return;
            }
            Value(val) => {
                command_override.push(os_to_string(val, "command argument"));
                // Collect remaining args as command
                for a in raw_args_or_exit(&mut parser) {
                    command_override.push(os_to_string(a, "command argument"));
                }
                break;
            }
            _ => {
                eprintln!("nyaibokkusu: unexpected argument");
                std::process::exit(1);
            }
        }
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| {
        eprintln!("nyaibokkusu: $HOME not set");
        std::process::exit(1);
    });
    let project_dir = std::env::current_dir()
        .unwrap_or_else(|e| {
            eprintln!("nyaibokkusu: cannot get cwd: {e}");
            std::process::exit(1);
        })
        .to_string_lossy()
        .into_owned();

    // Load configs
    let mut base = Config::default_base();
    let mut global = if no_global_config {
        None
    } else {
        Config::load_global(&home)
    };

    let mut project = Config::load_project(std::path::Path::new(&project_dir));

    // Expand ~ before merge so exclude_mounts matches
    base.expand_tilde(&home);
    if let Some(cfg) = global.as_mut() {
        cfg.expand_tilde(&home);
    }
    if let Some(cfg) = project.as_mut() {
        cfg.expand_tilde(&home);
    }
    cli.expand_tilde(&home);

    // Merge
    let merged = Config::merge(base, global, project, cli);

    // Determine command
    let cmd: Vec<String> = if !command_override.is_empty() {
        command_override
    } else if !merged.command.is_empty() {
        vec![merged.command.clone()]
    } else {
        vec!["bash".into()]
    };

    // Build bwrap args
    let bwrap_args = bwrap::build_args(&merged, &home, &project_dir);

    // exec bwrap
    let mut full_args = bwrap_args;
    full_args.push("--".into());
    full_args.extend(cmd);

    let err = Command::new("bwrap").args(&full_args).exec();
    eprintln!("nyaibokkusu: failed to exec bwrap: {err}");
    std::process::exit(1);
}

fn print_help() {
    println!(
        "\
Usage: nyaibokkusu [OPTIONS] [-- COMMAND [ARGS...]]

Whitelist-based bubblewrap sandbox for AI agents on NixOS.

Options:
  --map <path>      Add read-only bind mount
  --rw-map <path>   Add read-write bind mount
  --gpu             Bind GPU devices (DRI, NVIDIA, OpenGL)
  --display         Bind X11/Wayland sockets
  --docker          Bind Docker socket
  --no-global-config  Skip loading global config
  -h, --help        Show this help

Config files:
  $XDG_CONFIG_HOME/nyaibokkusu.toml (fallback: ~/.config/nyaibokkusu.toml)
  .nyaibokkusu.toml (per-project, merged on top)

Example:
  nyaibokkusu -- bash"
    );
}

fn value_string(parser: &mut lexopt::Parser, opt: &str) -> String {
    let val = parser.value().unwrap_or_else(|e| {
        eprintln!("nyaibokkusu: {opt} expects a value: {e}");
        std::process::exit(1);
    });
    os_to_string(val, opt)
}

fn os_to_string(val: OsString, label: &str) -> String {
    val.into_string().unwrap_or_else(|_| {
        eprintln!("nyaibokkusu: {label} must be valid UTF-8");
        std::process::exit(1);
    })
}

fn raw_args_or_exit(parser: &mut lexopt::Parser) -> Vec<OsString> {
    parser
        .raw_args()
        .unwrap_or_else(|e| {
            eprintln!("nyaibokkusu: {e}");
            std::process::exit(1);
        })
        .collect()
}
