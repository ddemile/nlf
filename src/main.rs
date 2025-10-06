use std::fs;

use crate::{parser::parse};

mod lexer;
mod parser;
mod interpreter;

fn main() {
    let contents = fs::read_to_string("src/program.nfl")
        .expect("Should have been able to read the file");

    let tokens = lexer::lex(contents);

    let json = serde_json::to_string_pretty(&tokens).unwrap();
    let _ = fs::write("tokens.json", json);

    println!("Generating ast");

    let ast = parse(tokens);

    let _ = fs::write("ast.json", serde_json::to_string_pretty(&ast).unwrap());

    interpreter::interpret(ast);
}
