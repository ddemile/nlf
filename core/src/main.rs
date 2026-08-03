use std::{env, path::Path, rc::Rc};

use nlf_core::{compiler::resolvers::{FileSystemModuleResolver, ModuleResolver}, errors::LanguageResult, loader::run_main, new_loader::Loader, tests, vm::{self, Value}};
use clap::Parser;
use nlf_macros::native_fn;
use nlf_shared::vm::VMContext;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    file: Option<String>,
    #[arg(short, long)]
    tests: bool,
    #[arg(short, long)]
    visit: bool,
    #[arg(long)]
    vm: bool,
    #[arg(long)]
    test_program: bool,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    extra: Vec<String>,
}

fn main() {
    let args = Args::parse();

    if args.test_program {
        vm::programs::run_imports_test_program();
        return;
    }

    if args.vm {
        let file = if let Some(value) = args.file { value } else { String::from("core/src/program/vm.nlf") };

        let resolver: Rc<dyn ModuleResolver> = Rc::new(FileSystemModuleResolver {
            process_path: env::current_dir().unwrap()
        });

        let loader = Loader::new(file, resolver);

        loader.run_main();

        return;
    }

    if args.tests {
        tests::run_tests(Path::new("core/tests"));
        return;
    }

    let file = if let Some(value) = args.file { value } else { String::from("core/src/program/main.nlf") };

    match run_main(&file) {
        Ok(_) => (),
        Err(e) => println!("{}", e.format(None))
    }
}