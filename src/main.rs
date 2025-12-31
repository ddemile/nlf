#![feature(box_patterns)]
#![feature(duration_millis_float)]
use std::{path::Path};

use crate::loader::run_main;
use clap::Parser;

mod lexer;
mod parser;
mod translator;
mod interpreter;
mod tests;
mod loader;
mod errors;
mod stdlib;

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
        tests::run_tests(Path::new("./tests"));
        return;
    }

    let file = if let Some(value) = args.file { value } else { String::from("src/program/main.nlf") };

    match run_main(&file) {
        Ok(_) => (),
        Err(e) => println!("{}", e.format(None))
    }
}