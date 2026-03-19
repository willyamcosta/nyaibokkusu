<p align="center">
  <img src="docs/icon.png" width="256" alt="NyaiBokkusu icon">
</p>

# nyaibokkusu

[bubblewrap](https://github.com/containers/bubblewrap) sandbox for agents tools for nixos.

inspired by [ai-jail](https://github.com/akitaonrails/ai-jail), it didn't work for me, because nix, and honestly, it was just too much noise to add nix support, so I didn't open a PR.

## quick start

```bash
# run a shell
nix run github:willyamcosta/nyaibokkusu

# run a command directly
nix run github:willyamcosta/nyaibokkusu -- claude

# install to your profile
nix profile install github:willyamcosta/nyaibokkusu

# remove from your profile
nix profile remove nyaibokkusu
```

## usage

```text
nyaibokkusu [--map PATH] [--rw-map PATH] [--gpu] [--display] [--docker] [--no-global-config] [-- COMMAND [ARGS...]]
```

## config

config files are merged in this order:

1. base defaults
1. global: `$XDG_CONFIG_HOME/nyaibokkusu.toml` (fallback `~/.config/nyaibokkusu.toml`)
1. project: `./.nyaibokkusu.toml`
1. CLI flags

example:

```toml
command = "claude"
exclude_mounts = ["~/.claude.json"]
gpu = true

[[mounts]]
path = "~/.extra"
rw = false

[env]
MY_VAR = "value"
```

## defaults & mounts

always included:

- core mounts (read-only): `/nix/store`, `/run/current-system`, `/etc/profiles`, `/etc/static`, `/etc/resolv.conf`, `/etc/hosts`, `/etc/ssl`, `/etc/nix`, `/etc/passwd`, `/etc/group`, `/etc/nsswitch.conf`, `~/.nix-profile`, `~/.local/state/nix`
- `/tmp` as tmpfs
- `$HOME` as tmpfs
- project directory as read-write bind mount (and set as working dir)
- nix profile mounts if present: (read-only)

default config mounts (only if they exist):

- `~/.claude`, `~/.claude.json`
- `~/.aider`, `~/.aider.conf.yml`
- `~/.codex`
- `~/.config/opencode`

everything else is not mounted unless you map it (for example `~/.ssh`, `~/.gnupg`, `~/.config`).

## credits

- [nixwrap](https://github.com/rti/nixwrap)
- [ai-jail](https://github.com/akitaonrails/ai-jail)
