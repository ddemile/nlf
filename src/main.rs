#![feature(box_patterns)]
#![feature(duration_millis_float)]
use std::{fs, path::Path};

use crate::{parser::parse};
use clap::Parser;

mod lexer;
mod parser;
mod translator;
mod interpreter;
mod tests;

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
        tests::run_tests(Path::new("./src/tests"));
        return;
    }

    let file = if let Some(value) = args.file { value } else { String::from("src/program.nlf") };

    let contents = fs::read_to_string(file)
        .expect("Should have been able to read the file");

    let tokens = lexer::lex(contents);

    let json = serde_json::to_string_pretty(&tokens).unwrap();
    let _ = fs::write("debug/tokens.json", json);

    let ast = parse(tokens);

    let _ = fs::write("debug/ast.json", serde_json::to_string_pretty(&ast).unwrap());

    let ir  = translator::translate(ast);

    let _ = fs::write("debug/ir.json", serde_json::to_string_pretty(&ir).unwrap());

    match interpreter::interpret(ir) {
        Ok(_) => (),
        Err(e) => println!("Runtime error: {}", e)
    }
}
