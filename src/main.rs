use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

/// Default location of the online package index.
/// Can be overridden with the `ARK_INDEX_URL` environment variable.
const DEFAULT_INDEX_URL: &str = "https://ark-repo.example.com/packages.json";

/// Local database of installed packages (`~` is expanded at runtime).
const INSTALLED_DB: &str = "~/.ark/list.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Package {
    name: String,
    version: u32,
    #[serde(default)]
    description: String,
    installed: bool,
    path: String,
    installation_command: String,
}

#[derive(Parser)]
#[command(name = "ark", version, about = "A tiny package manager driven by a web-hosted JSON index")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install a package from the repository
    Install { package: String },
    /// List installed packages
    List,
    /// Update all installed packages to the latest repository versions
    Update,
    /// Remove an installed package
    Remove { package: String },
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn index_url() -> String {
    std::env::var("ARK_INDEX_URL").unwrap_or_else(|_| DEFAULT_INDEX_URL.to_string())
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

fn installed_db_path() -> PathBuf {
    expand_tilde(INSTALLED_DB)
}

fn load_installed() -> Result<Vec<Package>, Box<dyn Error>> {
    let path = installed_db_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let json_text = fs::read_to_string(&path)?;
    let packages = serde_json::from_str(&json_text)?;
    Ok(packages)
}

fn save_installed(packages: &[Package]) -> Result<(), Box<dyn Error>> {
    let path = installed_db_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(packages)?;
    fs::write(&path, json)?;
    Ok(())
}

fn fetch_index() -> Result<Vec<Package>, Box<dyn Error>> {
    let response = reqwest::blocking::get(index_url())?;
    let packages = response.json::<Vec<Package>>()?;
    Ok(packages)
}

fn run_shell_command(command: &str) -> Result<(), Box<dyn Error>> {
    let status = Command::new("bash")
        .arg("-c")
        .arg(command)
        .status()?;

    if !status.success() {
        return Err(format!("command failed with status {status}: {command}").into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn install_package(name: &str) -> Result<(), Box<dyn Error>> {
    let index = fetch_index()?;

    let package = index
        .iter()
        .find(|item| item.name == name)
        .ok_or_else(|| {
            let available: Vec<&str> = index.iter().map(|p| p.name.as_str()).collect();
            format!(
                "package '{name}' not found in repository (available: {})",
                available.join(", ")
            )
        })?
        .clone();

    println!("Installing {} v{} ...", package.name, package.version);
    run_shell_command(&package.installation_command)?;

    // Replace any previous record of this package, then store it as installed.
    let mut installed = load_installed()?;
    installed.retain(|item| item.name != package.name);
    installed.push(Package {
        installed: true,
        ..package.clone()
    });
    save_installed(&installed)?;

    println!("Installed {} v{} at {}", package.name, package.version, package.path);
    Ok(())
}

fn remove_package(name: &str) -> Result<(), Box<dyn Error>> {
    let mut installed = load_installed()?;

    let position = installed
        .iter()
        .position(|item| item.name == name)
        .ok_or_else(|| format!("package '{name}' is not installed"))?;

    let package = installed.remove(position);

    let path = expand_tilde(&package.path);
    if path.is_dir() {
        fs::remove_dir_all(&path)?;
    } else if path.is_file() {
        fs::remove_file(&path)?;
    } else {
        println!("warning: path {} not found, removing record only", path.display());
    }

    save_installed(&installed)?;
    println!("Removed {}", package.name);
    Ok(())
}

fn list_packages() -> Result<(), Box<dyn Error>> {
    let installed = load_installed()?;

    if installed.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    println!("{:<20} {:<10} {}", "NAME", "VERSION", "PATH");
    for package in installed {
        println!("{:<20} {:<10} {}", package.name, package.version, package.path);
    }
    Ok(())
}

fn update_packages() -> Result<(), Box<dyn Error>> {
    let index = fetch_index()?;
    let installed = load_installed()?;
    let mut updated = 0;

    for package in &installed {
        if let Some(repo_package) = index.iter().find(|item| item.name == package.name) {
            if repo_package.version > package.version {
                println!(
                    "Updating {} from v{} to v{} ...",
                    package.name, package.version, repo_package.version
                );
                remove_package(&package.name)?;
                install_package(&repo_package.name)?;
                updated += 1;
            }
        }
    }

    if updated == 0 {
        println!("All packages are up to date.");
    } else {
        println!("Updated {updated} package(s).");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Install { package } => install_package(&package),
        Commands::List => list_packages(),
        Commands::Update => update_packages(),
        Commands::Remove { package } => remove_package(&package),
    };

    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
