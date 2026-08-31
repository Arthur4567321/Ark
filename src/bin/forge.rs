// ark-forge — Ark's defining extension (D: custom builds, F: unbreakable).
//
// Each package gets an Arch-style PKGBUILD recipe. USE flags selected by the
// user change how the recipe builds (via `has_flag`). Every build runs inside
// a sandbox (bubblewrap → unshare → refuse) and is transactional: staged →
// verified → atomically committed. Any failure leaves the system untouched.
//
// Install as:  ~/.ark/extensions/ark-forge   (ark's external-subcommand hook)
// Invoke as:   ark forge <package> [--flags a,b] [--no-sandbox] [--show-flags]

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const DEFAULT_INDEX: &str =
    "https://raw.githubusercontent.com/Arthur4567321/ark-repo/main/packages.json";
const LIST_PATH: &str = "~/.ark/list.json";

// ---------------------------------------------------------------------------
// small helpers

fn expand_tilde(path: &str) -> PathBuf {
    match std::env::var("HOME") {
        Ok(home) if path.starts_with("~/") => PathBuf::from(format!("{home}/{}", &path[2..])),
        _ => PathBuf::from(path),
    }
}

fn split_flags(s: &str) -> Vec<String> {
    s.split([',', ' ', '\t'])
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .map(str::to_string)
        .collect()
}

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

fn valid_flag(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-'))
}

// ---------------------------------------------------------------------------
// CLI

#[derive(Parser)]
#[command(
    name = "ark-forge",
    version,
    about = "Forge packages from PKGBUILD recipes: USE flags, sandboxed, transactional.",
    disable_help_subcommand = true
)]
struct Cli {
    /// Package name from the index, or a local directory containing a PKGBUILD
    package: String,

    /// Flags for this build (overrides package.flags and arkrc)
    #[arg(long, value_name = "A,B")]
    flags: Option<String>,

    /// Print declared + effective flags, then exit
    #[arg(long)]
    show_flags: bool,

    /// Disable sandboxing entirely (NOT recommended; loud + dangerous)
    #[arg(long)]
    no_sandbox: bool,

    /// Keep the staging directory after the run (for debugging recipes)
    #[arg(long)]
    keep_staging: bool,

    /// Package index URL (overrides $ARK_INDEX_URL, the same env var core ark uses)
    #[arg(long)]
    index: Option<String>,
}

// ---------------------------------------------------------------------------
// index + recipe

#[derive(Debug, Clone, Deserialize)]
struct IndexPackage {
    name: String,
    #[serde(default)]
    recipe: Option<String>,
}

fn http_get(url: &str) -> Result<String, Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(format!("GET {url} failed: {}", resp.status()).into());
    }
    Ok(resp.text()?)
}

fn download_recipe(index_url: &str, pkg: &str) -> Result<(String, String), Box<dyn Error>> {
    let index_text = http_get(index_url).map_err(|e| {
        format!("can't reach index at {index_url} ({e})\n  hint: is ark-repo pushed to GitHub? (dev override: ARK_INDEX_URL=…)")
    })?;
    let data: Vec<IndexPackage> = serde_json::from_str(&index_text)?;
    let entry = data
        .iter()
        .find(|p| p.name == pkg)
        .ok_or_else(|| format!("no package named '{pkg}' in the index"))?;

    let rel = entry
        .recipe
        .as_deref()
        .ok_or_else(|| format!("package '{pkg}' has no PKGBUILD recipe (missing \"recipe\" field)"))?;
    let base = match index_url.rfind('/') {
        Some(i) => &index_url[..i],
        None => ".",
    };
    let recipe_url = format!("{base}/{rel}");
    Ok((http_get(&recipe_url)?, recipe_url))
}

// ---------------------------------------------------------------------------
// PKGBUILD metadata (sourced by bash, parsed from its stdout)

struct Meta {
    name: String,
    version: u32,
    flags: Vec<String>,
    provides: Vec<String>,
}

fn extract_meta(pbuild: &Path) -> Result<Meta, String> {
    let script = r#"
source "$1" || exit 1
printf 'pkgname=%s\n' "$pkgname"
printf 'pkgver=%s\n' "$pkgver"
if declare -p flags >/dev/null 2>&1; then
    for f in "${flags[@]}"; do printf 'flag=%s\n' "$f"; done
fi
if declare -p provides >/dev/null 2>&1; then
    for p in "${provides[@]}"; do printf 'provides=%s\n' "$p"; done
fi
"#;
    let out = Command::new("bash")
        .arg("-c")
        .arg(script)
        .arg("_")
        .arg(pbuild)
        .output()
        .map_err(|e| format!("failed to run bash: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "PKGBUILD failed to load: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    let mut name = None;
    let mut version = None;
    let mut flags = Vec::new();
    let mut provides = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(v) = line.strip_prefix("pkgname=") {
            name = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("pkgver=") {
            version = v.trim().parse::<u32>().ok();
        } else if let Some(v) = line.strip_prefix("flag=") {
            flags.push(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("provides=") {
            provides.push(v.trim().to_string());
        }
    }

    let name = name.filter(|n| valid_name(n)).ok_or("PKGBUILD must set a valid pkgname")?;
    let version = version
        .ok_or("PKGBUILD pkgver must be a plain unsigned integer (ark's version scheme)")?;
    Ok(Meta { name, version, flags, provides })
}

// ---------------------------------------------------------------------------
// flag layering:  CLI --flags  >  ~/.ark/package.flags  >  ~/.ark/arkrc

fn global_flags() -> Vec<String> {
    let text = match fs::read_to_string(expand_tilde("~/.ark/arkrc")) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("FLAGS=") {
            return split_flags(v.trim_matches(['"', '\'']));
        }
    }
    Vec::new()
}

fn per_package_flags(pkg: &str) -> Option<Vec<String>> {
    let text = fs::read_to_string(expand_tilde("~/.ark/package.flags")).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        if parts.next() == Some(pkg) {
            return Some(parts.flat_map(split_flags).collect());
        }
    }
    None
}

fn resolve_flags(pkg: &str, declared: &[String], cli: &Option<String>) -> Vec<String> {
    let mut flags = match cli {
        Some(_) => split_flags(cli.as_deref().unwrap_or("")),
        None => per_package_flags(pkg).unwrap_or_else(global_flags),
    };
    flags.retain(|f| {
        if !valid_flag(f) {
            eprintln!("note: ignoring malformed flag '{f}'");
            return false;
        }
        if !declared.contains(f) {
            eprintln!("note: flag '{f}' is not declared by {pkg} — ignoring");
            return false;
        }
        true
    });
    flags.dedup();
    flags
}

// ---------------------------------------------------------------------------
// the build driver written into staging (runs INSIDE the sandbox)

fn runner_script(pkg: &str, flags: &[String]) -> String {
    format!(
        r#"#!/bin/bash
# Generated by ark-forge — do not edit.
set -euo pipefail
export ARK_PKG="{pkg}"
export ARK_FLAGS="{flags}"
has_flag() {{
    case ",$ARK_FLAGS," in *",$1,"*) return 0 ;; esac
    return 1
}}
source ./PKGBUILD
export srcdir="$PWD/work"
export pkgdir="$PWD/root"
mkdir -p "$srcdir" "$pkgdir"
phase() {{
    local name="$1"
    shift
    if ! declare -F "$name" >/dev/null; then return 0; fi
    ( cd "$srcdir" && "$name" ) || {{
        echo "ark-forge: recipe phase '$name' FAILED" >&2
        exit 1
    }}
}}
phase prepare
phase build
phase package
"#,
        flags = flags.join(",")
    )
}

// ---------------------------------------------------------------------------
// sandbox chain: bwrap → unshare userns → refuse (unless --no-sandbox)

fn preflight(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_bwrap(staging: &Path) -> Result<(), String> {
    let s = staging.to_string_lossy();
    let args: Vec<&str> = vec![
        "--unshare-net", "--unshare-ipc", "--unshare-pid", "--die-with-parent",
        "--tmpfs", "/",                       // empty root: nothing from the host leaks in
        "--ro-bind", "/usr", "/usr",
        "--ro-bind-try", "/etc", "/etc",
        "--symlink", "usr/lib", "/lib",
        "--symlink", "usr/lib64", "/lib64",
        "--symlink", "usr/bin", "/bin",
        "--symlink", "usr/bin", "/sbin",
        "--proc", "/proc",
        "--dev", "/dev",
        "--tmpfs", "/tmp",
        "--bind", &s, "/ark",                 // the ONLY writable place
        "--chdir", "/ark",
        "--clearenv",
        "--setenv", "HOME", "/ark",
        "--setenv", "PATH", "/usr/bin:/bin",
        "--setenv", "TMPDIR", "/tmp",
        "/usr/bin/bash", "runner.sh",
    ];
    let status = Command::new("bwrap").args(&args).status().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("build failed inside sandbox ({status})")) }
}

const UNSHARE_SCRIPT: &str = r#"
set -e
export PATH=/usr/bin:/bin
mount --make-rprivate / 2>/dev/null || true
mount -t tmpfs tmpfs /tmp
cd "$1"
export HOME="$PWD" TMPDIR=/tmp
exec /usr/bin/bash runner.sh
"#;

fn run_unshare(staging: &Path) -> Result<(), String> {
    let s = staging.to_string_lossy().to_string();
    let status = Command::new("unshare")
        .args(["-Ur", "-m", "--", "/usr/bin/bash", "-c", UNSHARE_SCRIPT, "ark-forge"])
        .arg(&s)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("build failed inside sandbox ({status})")) }
}

fn run_plain(staging: &Path) -> Result<(), String> {
    let status = Command::new("bash")
        .arg("runner.sh")
        .current_dir(staging)
        .env("HOME", staging)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("build failed ({status})")) }
}

fn run_build(staging: &Path, no_sandbox: bool) -> Result<&'static str, String> {
    if no_sandbox {
        eprintln!("⚠ --no-sandbox: recipe runs UNSANDBOXED. You have been warned.");
        run_plain(staging)?;
        return Ok("none (--no-sandbox)");
    }
    if preflight(
        "bwrap",
        &[
            "--tmpfs", "/",
            "--ro-bind", "/usr", "/usr",
            "--symlink", "usr/lib", "/lib",
            "--symlink", "usr/lib64", "/lib64",
            "--symlink", "usr/bin", "/bin",
            "--", "/usr/bin/true",
        ],
    ) {
        run_bwrap(staging)?;
        return Ok("bubblewrap");
    }
    if preflight("unshare", &["-Ur", "-m", "--", "/usr/bin/true"]) {
        eprintln!("note: bubblewrap unavailable — falling back to unshare (reduced isolation)");
        run_unshare(staging)?;
        return Ok("unshare user+mount ns");
    }
    Err(
        "no sandbox available (need bubblewrap or unprivileged user namespaces) \
         \n  install bubblewrap, or pass --no-sandbox to build unsandboxed"
            .into(),
    )
}

// ---------------------------------------------------------------------------
// transaction: verify provides → quarantine old → swap → record → clean up

#[derive(Serialize)]
struct ListEntry {
    name: String,
    version: u32,
    installed: bool,
    path: String,
    installation_command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    flags: Vec<String>,
}

fn update_list(entry: &ListEntry) -> Result<(), String> {
    let list_path = expand_tilde(LIST_PATH);
    // serde_json::Value keeps unknown fields of other entries intact
    let mut items: Vec<serde_json::Value> = match fs::read_to_string(&list_path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("list.json is corrupt: {e}"))?,
        Err(_) => Vec::new(),
    };
    items.retain(|v| v.get("name").and_then(|n| n.as_str()) != Some(entry.name.as_str()));
    items.push(serde_json::to_value(entry).map_err(|e| e.to_string())?);

    let tmp = list_path.with_extension("json.new");
    fs::write(&tmp, serde_json::to_string_pretty(&items).unwrap())
        .map_err(|e| format!("can't write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &list_path).map_err(|e| format!("atomic rename of list.json failed: {e}"))?;
    Ok(())
}

fn commit(
    meta: &Meta,
    flags: &[String],
    final_dir: &Path,
    root: &Path,
    staging: &Path,
    keep_staging: bool,
) -> Result<(), String> {
    let trash = expand_tilde(&format!("~/.ark/trash/{}.{}", meta.name, std::process::id()));
    fs::create_dir_all(trash.parent().unwrap()).map_err(|e| e.to_string())?;

    let had_old = final_dir.exists();
    if had_old {
        fs::rename(final_dir, &trash)
            .map_err(|e| format!("commit aborted: couldn't quarantine old package: {e}"))?;
    }
    if let Err(e) = fs::rename(root, final_dir) {
        if had_old {
            let _ = fs::rename(&trash, final_dir); // restore previous state
        }
        return Err(format!("commit aborted: couldn't move payload into place: {e}"));
    }

    let entry = ListEntry {
        name: meta.name.clone(),
        version: meta.version,
        installed: true,
        path: format!("~/.ark/packages/{}", meta.name),
        installation_command: format!("ark forge {}", meta.name),
        flags: flags.to_vec(),
    };
    if let Err(e) = update_list(&entry) {
        let _ = fs::remove_dir_all(final_dir);
        if had_old {
            let _ = fs::rename(&trash, final_dir);
        }
        return Err(format!("commit aborted while recording install: {e}"));
    }

    let _ = fs::remove_dir_all(&trash);
    if !keep_staging {
        let _ = fs::remove_dir_all(staging);
    }
    Ok(())
}

// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // 1. get the recipe: personal overlay (~/.ark/pkgs) → local dir → index
    let local_dir = PathBuf::from(&cli.package);
    let overlay = expand_tilde(&format!("~/.ark/pkgs/{}/PKGBUILD", cli.package));
    let (recipe_text, origin) = if valid_name(&cli.package) && overlay.exists() {
        (
            fs::read_to_string(&overlay)?,
            format!("{} (personal overlay — overrides the repo)", overlay.display()),
        )
    } else if local_dir.join("PKGBUILD").exists() {
        (fs::read_to_string(local_dir.join("PKGBUILD"))?, format!("{} (local dir)", local_dir.display()))
    } else {
        let index_url = cli
            .index
            .clone()
            .or_else(|| std::env::var("ARK_INDEX_URL").ok())
            .unwrap_or_else(|| DEFAULT_INDEX.to_string());
        let (text, url) = download_recipe(&index_url, &cli.package)?;
        (text, url)
    };

    // 2. staging area: ~/.ark/staging/<pkg>.<pid>/{work,root}
    let staging = expand_tilde(&format!("~/.ark/staging/stage.{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(staging.join("work"))?;
    fs::create_dir_all(staging.join("root"))?;
    fs::write(staging.join("PKGBUILD"), &recipe_text)?;

    // 3. read metadata + resolve flags (layered)
    let meta = extract_meta(&staging.join("PKGBUILD")).map_err(|e| {
        let _ = fs::remove_dir_all(&staging);
        format!("recipe {origin}: {e}")
    })?;
    let flags = resolve_flags(&meta.name, &meta.flags, &cli.flags);

    if cli.show_flags {
        println!("declared : {}", if meta.flags.is_empty() { "(none)".into() } else { meta.flags.join(", ") });
        println!("effective: {}", if flags.is_empty() { "(none)".into() } else { flags.join(", ") });
        println!("recipe   : {origin}");
        let _ = fs::remove_dir_all(&staging);
        return Ok(());
    }

    println!("forge {} {} (flags: [{}])", meta.name, meta.version, flags.join(","));
    println!("recipe   : {origin}");
    println!("staging  : {}", staging.display());

    // 4. run the build inside the sandbox
    fs::write(staging.join("runner.sh"), runner_script(&meta.name, &flags))?;
    let sandbox = match run_build(&staging, cli.no_sandbox) {
        Ok(s) => s,
        Err(e) => {
            if !cli.keep_staging {
                let _ = fs::remove_dir_all(&staging);
            }
            return Err(e.into());
        }
    };
    println!("sandbox  : {sandbox}");

    // 5. verify: every `provides` entry must exist in the payload
    let root = staging.join("root");
    for artifact in &meta.provides {
        if !root.join(artifact).exists() {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!(
                "verification failed: recipe declares provides=({}) but '{}' was not built — nothing was installed",
                meta.provides.join(" "),
                artifact
            ).into());
        }
    }
    if !meta.provides.is_empty() {
        println!("verified : {} artifact(s)", meta.provides.len());
    }

    // 6. commit (or leave everything untouched)
    let final_dir = expand_tilde(&format!("~/.ark/packages/{}", meta.name));
    commit(&meta, &flags, &final_dir, &root, &staging, cli.keep_staging)
        .map_err(|e| -> Box<dyn Error> {
            let _ = fs::remove_dir_all(&staging);
            format!("{e}\nark forge: FAILED — system state unchanged").into()
        })?;

    println!(
        "forged   : {} → {}",
        if flags.is_empty() { "ok".into() } else { format!("flags [{}]", flags.join(",")) },
        final_dir.display()
    );
    Ok(())
}
