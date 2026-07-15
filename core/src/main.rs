use std::path::Path;

use nlf_core::{loader::run_main, tests, vm};
use clap::Parser;

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
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    extra: Vec<String>,
}

fn main() {
    let args = Args::parse();

    if args.vm {
        if args.tests {
            vm::test();
            return;
        }

        let file = if let Some(value) = args.file { value } else { String::from("core/src/program/vm.nlf") };

        vm::test_compile(&file).unwrap();
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