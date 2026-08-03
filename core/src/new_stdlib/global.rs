use std::{io::{self, Write}, time::{SystemTime, UNIX_EPOCH}};

use nlf_macros::global;
use nlf_shared::{errors::LanguageResult, indexmap::IndexMap, vm::{Object, VMContext, Value}};

use crate::{new_addons::Addon, new_stdlib::MODULE_TABLE};

#[global]
pub fn print(value: Value, context: VMContext) {
    println!("{}", context.stringify(&value))
}

#[global]
pub fn assert(passed: Value, _: VMContext) {
    if !matches!(passed, Value::Bool(true)) {
        panic!("Assertion failed")
    }
}

#[global]
fn now(_: VMContext) -> LanguageResult<Value> {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("time should go forward");

    Ok(Value::Float(since_the_epoch.as_millis_f64()))
}

#[global]
fn binding(name: String, context: VMContext) -> LanguageResult<Value> {
    let table = MODULE_TABLE.lock();

    if !table.contains_key(name.as_str()) {
        panic!("Core module not found: {}", name)
    }

    let module_map = table.get(name.as_str()).unwrap().clone();

    let mut map: IndexMap<String, Value> = IndexMap::new();

    for (key, value) in module_map {
        let function_id = context.heap().allocate_native_function(value);

        map.insert(key.to_string(), Value::Native(function_id));
    }

    let object_id = context.heap().allocate_object(Object { map });

    Ok(Value::Object(object_id))
}

#[global]
fn input(prompt: String, context: VMContext) -> LanguageResult<Value> {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_goes_into_input_above) => {},
        Err(_no_updates_is_fine) => {},
    }

    let string_id = context.heap().allocate_string(input.trim().to_string());

    Ok(Value::String(string_id))
}

#[global]
fn confirm(prompt: Value, context: VMContext) -> LanguageResult<Value> {
    let Value::String(string_id) = prompt else {
        panic!("Expected string")
    };

    let prompt = context.get_string(string_id);
    
    loop {
        print!("{} [y/n]: ", prompt);
        io::stdout().flush().unwrap(); // ensure prompt shows immediately

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(Value::Bool(true)),
            "n" | "no" => return Ok(Value::Bool(false)),
            _ => println!("Please enter 'y' or 'n'."),
        }
    }
}

#[global]
fn addon(path: String, context: VMContext) -> LanguageResult<Value> {
    let addon = Addon::new(&path);

    let mut map: IndexMap<String, Value> = IndexMap::new();

    for (name, function) in addon.functions {
        let func = Value::Native(context.heap().allocate_native_function(function));

        map.insert(name, func);
    }

    Ok(Value::Object(context.heap().allocate_object(Object { map })))
}

#[global]
fn is_null(arg: Value, _: VMContext) -> LanguageResult<Value> {
    Ok(Value::Bool(matches!(arg, Value::Void)))
}

#[global]
fn number(string: String, _context: VMContext) -> LanguageResult<Value> {
    Ok(Value::Float(string.parse().unwrap()))
}

#[global]
fn panic(message: String, _context: VMContext) -> LanguageResult<Value> {
    panic!("{message}")
}