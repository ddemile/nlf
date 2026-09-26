use std::{collections::HashMap, env, fs, io::{self, Write}, path::{Path, PathBuf}, str::FromStr};

use anyhow::{Result, anyhow};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use reqwest::{StatusCode, Url};
use semver::Version;
use serde::{Deserialize, Serialize};
use tokio::spawn;

use crate::manifest::{Manifest, Package};

const REGISTRY_URL: &'static str = "http://0.0.0.0:3000";

#[derive(Deserialize, Debug)]
struct RegistryPackage {
    id: i64,
    name: String,
    version: Version,
}

pub async fn install_packages(packages: Vec<String>) -> Result<()> {
    let base_url = Url::from_str(REGISTRY_URL)?;

    let handles: Vec<_> = packages.into_iter().map(|package| {
        let base_url = base_url.clone();
        let task = spawn(async move {
            let url = base_url.join("packages/").unwrap().join(&package).unwrap();
            println!("Will install");

            match reqwest::get(url).await {
                Ok(response) => {
                    if response.status().is_success() {
                        let package: RegistryPackage = response.json().await.unwrap();

                        println!("Installing: {package:?}")
                    } else if response.status() == StatusCode::NOT_FOUND {
                        panic!("Package not found")
                    }
                },
                Err(err) => println!("{err:?}")
            }

            package
        });
        task
    }).collect();

    for handle in handles {
        match handle.await {
            Ok(package_name) => println!("Installed {package_name}", ),
            Err(err) => println!("{err}")
        }
    }

    Ok(())
}

pub fn init(path: PathBuf, package: Package) {
    let _ = fs::create_dir_all(".nlf/temp");
    let _ = fs::create_dir(".nlf/packages");

    let contents = "print(\"Hello world!\")";

    let _ = fs::create_dir("src");
    let _ = fs::write("src/main.nlf", contents);

    let manifest = Manifest {
        package,
        dependencies: HashMap::new()
    };

    let _ = fs::write(path.join("nlf.toml"), toml::to_string_pretty(&manifest).unwrap());
}

fn input(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_goes_into_input_above) => {},
        Err(_no_updates_is_fine) => {},
    }

    input.trim().to_string()
}

pub fn create_project() {
    let path = env::current_dir().unwrap();

    let manifest_path = path.join("nlf.toml");

    if manifest_path.exists() {
        println!("A manifest already exists, aborting...");
        return
    }

    let name = input("Project name: ");

    let package = Package {
        name,
        entry_point: String::from("src/main.nlf"),
        version: String::from("0.1.0")
    };

    init(path, package);
}