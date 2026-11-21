use std::{collections::HashMap, env, fs, io::{self, Write}, path::PathBuf, str::FromStr, sync::Arc};

use anyhow::{Result, anyhow};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use rayon::iter::ParallelIterator;
use reqwest::{Client, StatusCode, Url, multipart::{Form, Part}};
use semver::Version;
use serde::{Deserialize, Serialize};
use tar::{Archive, Builder};
use tokio::{fs::{File, remove_file}, io::AsyncWriteExt, spawn};
use futures_util::StreamExt;

use crate::manifest::{Manifest, Package, search_manifest};

const REGISTRY_URL: &'static str = "http://0.0.0.0:3000";

#[derive(Deserialize, Debug)]
struct RegistryPackage {
    id: i64,
    name: String,
    version: Version,
}

#[derive(Deserialize, Debug)]
struct RegistryVersion {
    name: String,
    package_id: String,
    artifact_id: String,
}

pub async fn install_packages(packages: Vec<String>) -> Result<()> {
    let manifest = search_manifest(env::current_dir().unwrap())?;

    let base_url = Url::from_str(REGISTRY_URL)?;

    let client = Arc::new(Client::new());

    let handles: Vec<_> = packages.into_iter().map(|package| {
        let manifest_location = manifest.location.clone();

        let client: Arc<Client> = client.clone();

        let base_url = base_url.clone();
        let task = spawn(async move {
            let project_root = manifest_location.parent().unwrap();
            let packages_folder = project_root.join(".nlf/packages/");
            let temp_folder = project_root.join(".nlf/temp/");

            let url = base_url.join(&format!("packages/{package}/versions/latest")).unwrap();

            match client.get(url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        let version: RegistryVersion = response.json().await.unwrap();

                        println!("Installing: {version:?}");

                        let artifact_url = base_url.join(&format!("packages/{package}/artifacts/{}", version.artifact_id)).unwrap();

                        let temp_path = temp_folder.join(version.artifact_id);

                        let mut file = File::create(&temp_path).await.unwrap();

                        let response = reqwest::get(artifact_url).await.unwrap();

                        let size: u32 = response.headers().get("Content-Length").unwrap().to_str().unwrap().parse().unwrap();

                        let mut stream = response.bytes_stream();

                        let mut current_size = 0_u32;

                        while let Some(item) = stream.next().await {
                            let item = item.unwrap();
                            tokio::io::copy(&mut item.as_ref(), &mut file).await.unwrap();

                            current_size += item.len() as u32;

                            println!("{:.2}%", current_size as f32 / size as f32 * 100.0)
                        }

                        let tar_gz = std::fs::File::open(&temp_path).unwrap();
                        let tar = GzDecoder::new(tar_gz);
                        let mut archive = Archive::new(tar);
                        archive.unpack(project_root.join(packages_folder).join(&package)).unwrap();
                        remove_file(temp_path).await.unwrap();
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

pub async fn publish_package() -> Result<()> {
    let results = search_manifest(env::current_dir().unwrap())?;

    let project_root = results.location.parent().ok_or(anyhow!("Failed to retrieve project root"))?;

    println!("Publishing {} v{}", results.manifest.package.name, results.manifest.package.version);

    let temp_file_path = format!("{}/.nlf/temp/{}.tar.gz", project_root.to_string_lossy(), results.manifest.package.name);

    let _ = fs::remove_file(&temp_file_path);

    let temp_file = std::fs::File::create_new(&temp_file_path)?;

    let encoder = GzEncoder::new(temp_file, Compression::best());

    let mut builder = Builder::new(encoder);
    builder.append_dir_all("", project_root)?;
    builder.finish()?;

    let encoder = builder.into_inner()?;
    encoder.finish()?;

    println!("Encoded");

    let base_url = Url::from_str(REGISTRY_URL)?;

    let url = base_url.join(&format!("packages/{}/artifacts", results.manifest.package.name))?;

    let part = Part::file(&temp_file_path).await.unwrap();

    let form = Form::new()
        .part("file", part);

    let client = Client::new();

    let artifact_id = client.post(url)
        .multipart(form)
        .send()
        .await?
        .text()
        .await?;

    let _ = fs::remove_file(temp_file_path);

    let url = base_url.join(&format!("packages/{}/versions", results.manifest.package.name))?;

    #[derive(Serialize)]
    struct CreateVersionPayload {
        name: String,
        artifact_id: String
    }

    let response = client.post(url)
        .json(&CreateVersionPayload {
            name: results.manifest.package.version,
            artifact_id
        })
        .send()
        .await?;

    if !response.status().is_success() {
        println!("{}", response.status());
        return Err(anyhow!("[Request failed] Failed to publish version"))
    }

    Ok(())
}