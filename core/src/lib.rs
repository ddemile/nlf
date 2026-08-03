#![feature(box_patterns)]
#![feature(duration_millis_float)]

pub mod lexer;
pub mod parser;
pub mod vm;
pub mod tests;
pub mod loader;
pub mod errors;
pub mod stdlib;
pub mod addons;
pub mod explorer;
pub mod analysis;
pub mod type_checker;
pub mod type_resolver;
pub mod compiler;
pub mod translator;
