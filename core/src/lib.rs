#![feature(box_patterns)]
#![feature(duration_millis_float)]

pub mod lexer;
pub mod parser;
pub mod translator;
pub mod interpreter;
pub mod tests;
pub mod loader;
pub mod errors;
pub mod stdlib;
pub mod addons;