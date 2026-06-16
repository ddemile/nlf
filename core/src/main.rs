use std::path::Path;

use nlf_core::{analysis::{ScopeBuilder, ScopeId, SymbolIndex}, explorer::{self}, lexer, loader::{Module, run_main}, parser, tests};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    file: Option<String>,
    #[arg(short, long)]
    tests: bool,
    #[arg(short, long)]
    visit: bool,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    extra: Vec<String>,
}

fn main() {
    let args = Args::parse();

    if args.tests {
        tests::run_tests(Path::new("core/tests"));
        return;
    }

    if args.visit {
        let mut scope_builder = ScopeBuilder::new();

        let path = Module::resolve_path("core/src/program/visit.nlf").unwrap();

        let code = std::fs::read_to_string(&path).unwrap();

        let tokens = lexer::lex(code).unwrap();
        let program = parser::parse(tokens).unwrap();

        explorer::visit_program(&program, &mut scope_builder);

        let index = SymbolIndex::from(scope_builder);

        index.scopes.iter().enumerate().for_each(|(i, scope)| {
            println!("Scope {}: Parent: {:?}, Children: {:?}, Symbols: {:?}, Span: {:?}", i, scope.parent, scope.children, scope.symbols, scope.span);
        });

        index.symbol_at(8).iter().for_each(|symbol| {
            println!("Symbol: {} (Scope: {:?})", symbol.name, symbol.scope);
        });

        return;
    }

    let file = if let Some(value) = args.file { value } else { String::from("core/src/program/main.nlf") };

    match run_main(&file) {
        Ok(_) => (),
        Err(e) => println!("{}", e.format(None))
    }
}