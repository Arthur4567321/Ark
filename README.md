# Ark 📦

A tiny Rust package manager driven by a web-hosted JSON index.

## Build

```bash
cargo build --release
```

## Hosting on GitHub

The CLI and the emerge extension fetch the index from GitHub raw by default:

```
https://raw.githubusercontent.com/Arthur4567321/Ark/main/web/packages.json
```

To publish (from the repo root):

```bash
gh repo create Arthur4567321/Ark --public --source=. --push   # or create it on github.com and:
# git remote add origin git@github.com:Arthur4567321/Ark.git
# git push -u origin main
```

Once pushed, anyone can install packages with no local server running — the
`emerge` extension itself is installed the same way (`ark install emerge`
downloads it from the same repo). Override the index location anytime with:

```bash
ARK_INDEX_URL=http://localhost:8000/packages.json ark list   # local testing
```

Note: `raw.githubusercontent.com` caches files for ~5 minutes — newly pushed
index changes may take a few minutes to appear.

## Repository web page

The `web/` directory is the package repository:

- `index.html` — searchable web page that renders the package catalog
- `packages.json` — the JSON index the CLI downloads (includes `installation_command` per package)

Serve it locally:

```bash
cd web
python3 -m http.server 8000
# then open http://localhost:8000
```

The CLI fetches `http://localhost:8000/packages.json` (see `INDEX_URL` in `src/main.rs` — change it to your real hosted URL).

## Usage

```bash
ark install <package>   # install a package from the index
ark list                # list installed packages
ark update              # reinstall packages whose repo version is newer
ark remove <package>    # remove an installed package and its files
```

Installed packages are recorded in `~/.ark/list.json` (the `~` is expanded to `$HOME` at runtime).

## Extensions

Any executable named `ark-<name>` in `~/.ark/extensions/` becomes a subcommand: `ark <name> ...` gets dispatched to it with the remaining arguments.

**Bundled extension: `emerge`** — Gentoo-style USE flags with Debian-grade safety:

```bash
ark install emerge                      # installs the extension from this repo
ark emerge install hello-gentoo +color +quotes
ark emerge install super-cow           # auto-installs the 'hello' dependency
ark emerge remove hello                # REFUSED: super-cow depends on it
ark emerge remove hello --force        # allowed, goes to trash not /dev/null
ark emerge rollback                    # undo the last operation
ark emerge update -p                   # pretend dry-run, Gentoo style
ark emerge list / world / search / info / backups
```

Safety features: dpkg-style lock (no concurrent runs), atomic database writes, a snapshot before every change (last 5 kept), removals moved to trash instead of deleted, automatic rollback if an install fails, and dependency-aware removal refusal.

USE flags are stored per package in `~/.ark/emerge/package.use/<name>`; packages opt into flags via a `use_flags` list and declare dependencies via `depends` in `packages.json`.

## Package JSON schema

```json
{
  "name": "hello",
  "version": 1,
  "installed": false,
  "path": "~/.ark/packages/hello",
  "installation_command": "bash command that installs the package at <path>"
}
```

- `version` — compared numerically by `ark update`
- `path` — what `ark remove` deletes (`rm -rf`)
- `installation_command` — run through `bash -c` by `ark install`
