#![feature(box_patterns)]
#![feature(duration_millis_float)]
use std::path::Path;

use nlf_core::{tests, loader::run_main};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    file: Option<String>,
    #[arg(short, long)]
    tests: bool
}

fn main() {
    let args = Args::parse();

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