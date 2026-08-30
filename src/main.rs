use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::process::Command;

// Web-hosted package index (serve web/ with: python3 -m http.server 8000)
const INDEX_URL: &str = "http://localhost:8000/packages.json";
const LIST_PATH: &str = "list.json";

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
}

fn install_package(package: String) -> Result<(), Box<dyn Error>> {
    let response = reqwest::blocking::get(INDEX_URL)?; // GET("") is not a real function

    let data: Vec<Package> = response.json()?; // json() returns a Result, needs `?`

    // find() returns an Option -> unwrap it; structs use .name, not ["name"]
    let result = data.iter().find(|item| item.name == package).unwrap();

    // .status() actually runs the command and checks the exit code
    Command::new("bash")
        .arg("-c")
        .arg(&result.installation_command)
        .status()?;

    // first run has no list.json yet -> start from an empty list
    let mut installed_packages: Vec<Package> = match fs::read_to_string(LIST_PATH) {
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

    fs::write(LIST_PATH, json)?;

    Ok(())
}

fn list_packages() {
    let json_text = fs::read_to_string(LIST_PATH).unwrap(); // read_file() doesn't exist
    let data: Vec<Package> = serde_json::from_str(&json_text).unwrap(); // parse_json() doesn't exist

    for item in &data {
        println!("{}", item.name);
    }
}

fn remove_package(package: String) {
    let json_text = fs::read_to_string(LIST_PATH).unwrap();
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
    fs::write(LIST_PATH, serde_json::to_string_pretty(&remaining).unwrap()).unwrap();
}

fn update_packages() {
    let response_dataset = reqwest::blocking::get(INDEX_URL).unwrap();
    let data: Vec<Package> = response_dataset.json().unwrap();

    let json_text = fs::read_to_string(LIST_PATH).unwrap();
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

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        // match arms need `,` not `;`
        Commands::Install { package } => install_package(package)?,
        Commands::List => list_packages(),
        Commands::Update => update_packages(),
        Commands::Remove { package } => remove_package(package), // argument was missing
    }

    Ok(())
}
