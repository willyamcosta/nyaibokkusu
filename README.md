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

environment variables in paths are expanded:

- `$VAR` - replaced with the value of VAR (empty string if unset)
- `${VAR}` - same as `$VAR`
- `${VAR:-default}` - replaced with VAR value, or `default` if unset
- `~` - replaced with $HOME (only at path start or in defaults)
- multiple variables are expanded: `$A/$B` → `/path_a/path_b`
- recursive expansion is NOT supported: `${${VAR}}` literal

### security note

paths in config files are expanded using environment variables from the host
system. do not use `.nyaibokkusu.toml` files from untrusted sources, as they
could expose sensitive environment variables through paths.

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
- `/nix/var/nix/daemon-socket` (read-write, if present) — lets nix commands talk to the daemon
- `/nix/var/nix/profiles` (read-only, if present)
- `NIX_REMOTE=daemon` is set when the daemon socket exists so nix uses the daemon instead of a chroot store
- `/tmp` as tmpfs
- `$HOME` as tmpfs
- project directory as read-write bind mount (and set as working dir)

default config mounts (only if they exist):

- `~/.claude`, `~/.claude.json`
- `~/.aider`, `~/.aider.conf.yml`
- `~/.codex`
- opencode: `$XDG_CONFIG_HOME/opencode`, `$XDG_CACHE_HOME/opencode`, `$XDG_DATA_HOME/opencode`, `$XDG_STATE_HOME/opencode` (defaults follow XDG spec)

everything else is not mounted unless you map it (for example `~/.ssh`, `~/.gnupg`, `~/.config`).

## common recipes

**opencode with permissive config**

```nix
let
  permissive-config = pkgs.writeText "opencode.json" ''
    { "$schema": "https://opencode.ai/config.json", "permission": { "*": "allow" } }
  '';
in
pkgs.writeShellScriptBin "nyai-opencode" ''
  exec ${nyaibokkusu}/bin/nyaibokkusu \
    --map ${permissive-config}:~/.config/opencode/opencode.json \
    -- opencode "$@"
''
```

**mount your whole `~/.config` (with optional exclusions)**

```toml
exclude_mounts = ["~/.config/secret-app"]

[[mounts]]
path = "~/.config"
```

**mount specific dirs, e.g., git identity/config**

```toml
# ~/.config/nyaibokkusu.toml
[[mounts]]
path = "~/.gitconfig"

[[mounts]]
path = "~/.config/git"
```

## credits

- [nixwrap](https://github.com/rti/nixwrap)
- [ai-jail](https://github.com/akitaonrails/ai-jail)
