use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

// Package index hosted on GitHub (override with the ARK_INDEX_URL env var)
const INDEX_URL: &str =
    "https://raw.githubusercontent.com/Arthur4567321/Ark/main/web/packages.json";

fn index_url() -> String {
    std::env::var("ARK_INDEX_URL").unwrap_or_else(|_| INDEX_URL.to_string())
}
const LIST_PATH: &str = "~/.ark/list.json";

// "~" is only expanded by shells, not by fs APIs -> resolve it to $HOME
fn expand_tilde(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if path.starts_with("~/") => format!("{home}/{}", &path[2..]),
        _ => path.to_string(),
    }
}

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Package {
    name: String,
    version: u32,
    installed: bool,
    path: String,
    installation_command: String,
}

#[derive(Subcommand)]
enum Commands {
    Install { package: String },
    List,
    Update,
    Remove { package: String },
    #[command(external_subcommand)]                                               
    External(Vec<String>),
}

fn install_package(package: String) -> Result<(), Box<dyn Error>> {
    let response = reqwest::blocking::get(index_url())?; // GET("") is not a real function

    let data: Vec<Package> = response.json()?; // json() returns a Result, needs `?`

    // find() returns an Option -> unwrap it; structs use .name, not ["name"]
    let result = data.iter().find(|item| item.name == package).unwrap();

    // .status() actually runs the command and checks the exit code
    Command::new("bash")
        .arg("-c")
        .arg(&result.installation_command)
        .status()?;

    // first run has no list.json yet -> start from an empty list
    let mut installed_packages: Vec<Package> = match fs::read_to_string(expand_tilde(LIST_PATH)) {
        Ok(json_text) => serde_json::from_str(&json_text)?,
        Err(_) => Vec::new(),
    };

    installed_packages.push(Package {
        name: result.name.clone(), // missing commas added
        version: result.version,
        installed: true,
        path: result.path.clone(),
        installation_command: result.installation_command.clone(),
    }); // missing semicolon added

    let json = serde_json::to_string_pretty(&installed_packages)?;
    let list_path = expand_tilde(LIST_PATH);
    if let Some(parent) = Path::new(&list_path).parent() {
        fs::create_dir_all(parent)?; // ~/.ark may not exist yet
    }

    fs::write(&list_path, json)?;

    Ok(())
}

fn list_packages() {
    let json_text = fs::read_to_string(expand_tilde(LIST_PATH)).unwrap(); // read_file() doesn't exist
    let data: Vec<Package> = serde_json::from_str(&json_text).unwrap(); // parse_json() doesn't exist

    for item in &data {
        println!("{}", item.name);
    }
}

fn remove_package(package: String) {
    let json_text = fs::read_to_string(expand_tilde(LIST_PATH)).unwrap();
    let data: Vec<Package> = serde_json::from_str(&json_text).unwrap();

    let result = data.iter().find(|item| item.name == package).unwrap();

    // bash -c takes ONE command string; "rm" "-rf" path as separate args were ignored
    Command::new("bash")
        .arg("-c")
        .arg(format!("rm -rf {}", result.path))
        .status()
        .unwrap();

    // also drop the record from list.json
    let remaining: Vec<Package> = data.into_iter().filter(|item| item.name != package).collect();
    fs::write(expand_tilde(LIST_PATH), serde_json::to_string_pretty(&remaining).unwrap()).unwrap();
}

fn update_packages() {
    let response_dataset = reqwest::blocking::get(index_url()).unwrap();
    let data: Vec<Package> = response_dataset.json().unwrap();

    let json_text = fs::read_to_string(expand_tilde(LIST_PATH)).unwrap();
    let installed_packages: Vec<Package> = serde_json::from_str(&json_text).unwrap();

    // `for a in x && b in y` is invalid syntax -> nested loops
    for item in &data {
        for package in &installed_packages {
            // compare only matching package names, not every pair
            if (item.version > package.version) && item.name == package.name {
                remove_package(package.name.clone());
                install_package(item.name.clone()).unwrap();
            }
        }
    }
}

fn run_external_command(name: &str, args: &[String]) -> Result<(), String> {
    // "~/..." is only expanded by shells -> resolve it to $HOME
    let path = expand_tilde(&format!("~/.ark/extensions/ark-{name}"));

    if !Path::new(&path).exists() {
        // was missing `return`: the error was built and thrown away
        return Err(format!("No extension named: {name}"));
    }

    // run the extension directly with its args (no bash string concat,
    // so arguments with spaces survive); report a failing exit code
    let status = Command::new(&path)
        .args(args)
        .status()
        .map_err(|e| e.to_string())?;

    if !status.success() {
        return Err(format!("ark-{name} failed with status {status}"));
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        // match arms need `,` not `;`
        Commands::Install { package } => install_package(package)?,
        Commands::List => list_packages(),
        Commands::Update => update_packages(),
        Commands::Remove { package } => remove_package(package),// argument was missing
        Commands::External(args) => {
            let name = &args[0];
            let rest = &args[1..];
            run_external_command(name, rest)?;
        }
    };

    
    Ok(())
}
