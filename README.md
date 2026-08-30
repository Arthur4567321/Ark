# Ark 📦

A tiny Rust package manager driven by a web-hosted JSON index.

## Build

```bash
cargo build --release
```

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

Installed packages are recorded in `list.json` in the current directory.

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
