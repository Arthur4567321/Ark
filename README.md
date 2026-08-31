# Ark 📦

A tiny Rust package manager driven by a web-hosted JSON index — with a defining
extension: **`ark forge`**. Every package is a customizable PKGBUILD recipe;
USE flags change how it builds, the build runs in a sandbox, and the install is
transactional — ark refuses to leave you in a broken state.

This repository contains **only the manager itself**. The package repository
lives separately in `~/ark-repo` (see below) — the manager's code and the
packages it ships never mix, like pacman vs. its mirrors or portage vs. the
ebuild repo.

## Build

```bash
cargo build --release
```

This produces two binaries:

- `ark` — the core CLI
- `ark-forge` — the forge extension; install it with:

```bash
mkdir -p ~/.ark/extensions
cp target/release/ark-forge ~/.ark/extensions/ark-forge
```

## The package repository (`~/ark-repo`)

A standalone directory, its own git repo, intentionally outside this project:

```
~/ark-repo                      →  github.com/Arthur4567321/ark-repo
├── packages.json               # the index (fetched from GitHub raw)
├── index.html                  # searchable web catalog (USE-flag chips)
├── pkgs/
│   └── <name>/PKGBUILD         # one Arch-style recipe per package
└── extensions/                 # installable extensions (ark-forge, ark-emerge)
```

**Hosting is git-only** — no server anywhere. Both `ark` and `ark-forge`
default to:

```
https://raw.githubusercontent.com/Arthur4567321/ark-repo/main/packages.json
```

Publishing package changes is just a push:

```bash
cd ~/ark-repo && git add -A && git commit -m "…" && git push
```

(`raw.githubusercontent.com` caches for ~5 minutes — fresh pushes may take a
moment to appear.)

For local development without pushing, serve a checkout and override:

```bash
python3 -m http.server 8000 --directory ~/ark-repo
ARK_INDEX_URL=http://localhost:8000/packages.json ark forge hello
```

## Usage

```bash
ark install <package>   # install a package from the index (legacy path)
ark list                # list installed packages
ark update              # reinstall packages whose repo version is newer
ark remove <package>    # remove an installed package and its files
ark forge <package>     # 🔥 THE defining extension (see below)
```

Installed packages are recorded in `~/.ark/list.json` (the `~` is expanded to `$HOME` at runtime).

## ark forge — the defining extension

Every recipe-capable package in the repository ships an Arch-style **PKGBUILD**
(`web/pkgs/<name>/PKGBUILD`). USE flags selected by the user change how the
recipe builds. Every build is sandboxed and transactional.

```bash
ark forge hello                                  # build with your configured flags
ark forge hello --flags minimal                  # one-shot flags
ark forge hello-gentoo --flags color,fancy,quotes
ark forge hello --show-flags                     # declared + effective flags
ark forge ~/ark-repo/pkgs/hello                  # build from an explicit recipe dir
ark forge hello --no-sandbox                     # escape hatch (loud + dangerous)
ark forge hello --keep-staging                   # debugging aid
```

### Recipe lookup order

1. **`~/.ark/pkgs/<name>/PKGBUILD`** — your personal overlay; wins over the
   repo, perfect for hacking on a recipe without touching any git
2. an explicit directory argument containing a `PKGBUILD`
3. the recipe URL published in the index

### Flag layering (most specific wins)

1. `--flags a,b` on the command line
2. `~/.ark/package.flags` — one line per package: `hello minimal`
3. `~/.ark/arkrc` — global defaults: `FLAGS="minimal"`

Unknown flags are warned about and ignored.

### PKGBUILD recipe format

```bash
pkgname=hello
pkgver=2                    # plain integer, ark's version scheme
flags=(minimal)             # USE flags this recipe understands
provides=(bin/hello)        # artifacts the build MUST produce (verified!)

build() {
    if has_flag minimal; then ... else ... fi   # flags bake in at BUILD time
}
package() {
    install -Dm755 hello "$pkgdir/bin/hello"    # $pkgdir = staging payload
}
```

Phases (`prepare`, `build`, `package`) are optional and run with `$srcdir` /
`$pkgdir` set, Arch-style. **Never call `has_flag` at runtime** — flags exist
only during the build; bake the decision into the artifact.

### The transaction (why ark can't leave you broken)

1. recipe downloaded to a private staging dir (`~/.ark/staging/`)
2. build runs in a sandbox: **bubblewrap** (no network, tmpfs root, staging is
   the only writable path) → **unshare** user+mount ns fallback → **refuse**
3. every entry in `provides` must exist in the payload, or nothing is installed
4. atomic commit: old version quarantined, payload swapped in, `list.json`
   rewritten via temp-file rename — a failure at any point restores the
   previous state

### The catalog: real distro software

Most packages in the repo wrap **the real binaries your distro already
ships** (`/usr/bin/rg`, `/usr/bin/nvim`, …) — the forge sandbox has no
network by design, so ark builds on top of your distro instead of
replacing it. Recipes verify the distro binary exists at build time and
refuse cleanly (nothing installed) if it doesn't. USE flags bake real
behavior into the wrapper:

- `ark forge ripgrep --flags smart` → `rg --smart-case`
- `ark forge fd --flags hidden,noignore` → `fd --hidden --no-ignore-vcs`
- `ark forge eza --flags git,icons` → `eza --git --icons=auto`
- `ark forge curl --flags retry` → `curl --retry 3`
- `ark forge neovim --flags config` → ships a sane `init.vim` in the package
- `ark forge zsh --flags config` → `ZDOTDIR` points into the package

Catalog: ripgrep, fd, bat, eza, fzf, tree, curl, wget, neovim, vim, nano,
micro, btop, zsh, starship, jq, unzip, duf, sd (+ the original toy packages).

The **ark-forge extension itself** is installable from the repo's
`extensions/` directory:

```bash
ark install ark-forge     # curls it from the index host into ~/.ark/extensions/
```

(`ark install ark-forge` curls the binary straight from the repo's GitHub raw
URL — works from any machine, no server running.)

## Extensions

Any executable named `ark-<name>` in `~/.ark/extensions/` becomes a subcommand: `ark <name> ...` gets dispatched to it with the remaining arguments. `ark-forge` (built from this repo, see above) is the flagship.

**Legacy extension: `emerge`** — the first USE-flag experiment; operates at
runtime instead of build time. Superseded by `ark forge`, kept for reference:

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
  "version": 2,
  "recipe": "pkgs/hello/PKGBUILD",
  "flags": ["minimal"],
  "path": "~/.ark/packages/hello",
  "installation_command": "ark forge hello"
}
```

- `version` — compared numerically by `ark update`
- `recipe` — PKGBUILD path (relative to the index URL) used by `ark forge`
- `flags` — declared USE flags (shown as chips on the web page)
- `path` — what `ark remove` deletes (`rm -rf`)
- `installation_command` — run through `bash -c`; forged packages point at
  `ark forge <name>` so even `ark update` goes through the sandbox
