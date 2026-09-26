use std::{env, path::Path, rc::Rc};

use nlf_core::{compiler::resolvers::{FileSystemModuleResolver, ModuleResolver}, loader::Loader, manifest::{ManifestSearchResult, search_manifest}, package_manager::{create_project, install_packages}, vm};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    file: Option<String>,
    #[arg(long)]
    test_program: bool,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    extra: Vec<String>,
    #[command(subcommand)]
    command: Option<Commands>
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[cfg(not(target_arch = "wasm32"))]
    #[command(name = "run-tests")]
    Tests,
    #[command(aliases = ["i", "add"])]
    Install {
        packages: Vec<String>
    },
    #[command(aliases = ["uni", "remove", "rm"])]
    Uninstall {
        packages: Vec<String>
    },
    Init
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Some(command) = cli.command {
        match command {
            #[cfg(not(target_arch = "wasm32"))]
            Commands::Tests => {
                if cfg!(not(target_arch = "wasm32")) {
                    nlf_core::tests::run_tests(Path::new("core/tests"));
                    return;
                } else {
                    panic!("Tests are not supported in the browser")
                }
            },
            Commands::Install { packages } => {
                let _ = install_packages(packages).await;
            }
            Commands::Uninstall { packages } => {
                todo!()
            },
            Commands::Init => {
                create_project();
            }
        }
        return;
    }

    if cli.test_program {
        vm::programs::run_imports_test_program();
        return;
    }

    let mut file = if let Some(value) = cli.file { value } else { String::from("core/src/program/main.nlf") };

    let mut process_path = env::current_dir().unwrap();

    if let Ok(ManifestSearchResult { manifest, location }) = search_manifest(env::current_dir().unwrap()) {
        // If we run `nlf .`, launch the entry point defined in nlf.toml
        if file == "." {
            file = manifest.package.entry_point;
            process_path = location.parent().unwrap().to_path_buf();
        }
    }

    let resolver: Rc<dyn ModuleResolver> = Rc::new(FileSystemModuleResolver {
        process_path
    });

    let loader = Loader::new(file, resolver);

    loader.run_main();
}