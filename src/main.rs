#![feature(box_patterns)]
#![feature(duration_millis_float)]
use std::fs;

use crate::{parser::parse};
use clap::Parser;

mod lexer;
mod parser;
mod translator;
mod interpreter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    file: Option<String>
}

fn main() {
    let args = Args::parse();

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

    interpreter::interpret(ir);

}
