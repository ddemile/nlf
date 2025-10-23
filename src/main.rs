#![feature(box_patterns)]
#![feature(duration_millis_float)]
use std::{env, path::Path};
use inline_colorization::*;

use crate::loader::run_main;
use clap::Parser;

mod lexer;
mod parser;
mod translator;
mod interpreter;
mod tests;
mod loader;

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

    let file = if let Some(value) = args.file { value } else { String::from("src/program.nlf") };

    match run_main(&file) {
        Ok(_) => (),
        Err(e) => println!("{color_red}Runtime error{color_reset}: {}", e)
    }
}
