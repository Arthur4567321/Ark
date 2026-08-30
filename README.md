# Ark 📦

A tiny Rust package manager driven by a web-hosted JSON index.

## Build

```bash
cargo build --release
```

## Repository web page

The `web/` directory is a self-contained package repository:

- `index.html` — searchable web page that renders the package catalog
- `packages.json` — the JSON index the CLI downloads

Serve it locally:

```bash
cd web
python3 -m http.server 8000
# then open http://localhost:8000
```

Point the CLI at it:

```bash
export ARK_INDEX_URL=http://localhost:8000/packages.json
```

(Without the variable, `DEFAULT_INDEX_URL` in `src/main.rs` is used — change it to your real hosted URL.)

## Usage

```bash
ark install <package>   # install a package from the index
ark list                # list installed packages
ark update              # reinstall packages whose repo version is newer
ark remove <package>    # remove an installed package and its files
```

Installed packages are recorded in `~/.ark/list.json`.

## Package JSON schema

```json
{
  "name": "hello",
  "version": 1,
  "description": "Classic hello-world demo binary.",
  "installed": false,
  "path": "~/.ark/packages/hello",
  "installation_command": "bash -c command that installs the package at <path>"
}
```

- `version` — compared numerically by `ark update`
- `path` — where `ark remove` deletes files (`~` is expanded to `$HOME`)
- `installation_command` — run through `bash -c` by `ark install`
