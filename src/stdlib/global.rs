use std::{cell::RefCell, collections::HashMap, io::{self, Write}, rc::Rc, sync::Arc, time::{SystemTime, UNIX_EPOCH}};

use lib::expose;

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::{BuiltInFunction, FunctionKind, ObjectRef, ValueHolder}, stdlib::MODULE_TABLE, argument};

#[expose]
fn print(values: &[ValueHolder], context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    let arguments: Vec<String> = values
        .iter()
        .map(|argument| -> String {
            if let ValueHolder::Object(object_ref) = argument {
                return serde_json::to_string(&object_ref.fetch(context.clone())).expect("Failed to parse object");
            } else if let ValueHolder::Array(array_ref) = argument {
                return serde_json::to_string(&array_ref.fetch(context.clone())).expect("Failed to parse array");
            }

            format!("{argument}")
        })
        .collect();

    println!("{}", arguments.join(" "));

    Ok(ValueHolder::Void)
}

#[expose]
fn panic(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    let message = argument!(values, ValueHolder::String, "message", 0);

    Err(LanguageError::with_source(RuntimeError::Custom(message.clone()), 0, 0))
}

#[expose]
fn now(_values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("time should go forward");
    Ok(ValueHolder::Float(since_the_epoch.as_millis_f64()))
}

#[expose]
fn assert(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    let a = match values.get(0).unwrap() {
        ValueHolder::Bool(f) => f,
        _ => panic!("Expected bool value"),
    };

    if !a {
        return Err(LanguageError::with_source(RuntimeError::Custom("Assertion failed".to_string()), 0, 0));
    }

    Ok(ValueHolder::Void)
}

#[expose]
fn test(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    let name = match values.get(0).unwrap() {
        ValueHolder::String(f) => f,
        _ => panic!("Expected test name"),
    };

    println!("Running test: {}", name);

    Ok(ValueHolder::Int(4))
}

#[expose]
fn binding(values: &[ValueHolder], context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    let name = match values.get(0).unwrap() {
        ValueHolder::String(f) => f,
        _ => panic!("Expected test name"),
    };

    let table = MODULE_TABLE.lock().unwrap();

    if !table.contains_key(name.as_str()) {
        return Err(LanguageError::from(RuntimeError::Custom(format!("Core module not found: {}", name))))
    }

    let module_map = table.get(name.as_str()).unwrap().clone();

    let mut map: HashMap<String, ValueHolder> = HashMap::new();

    for (key, value) in module_map {
        let function = ValueHolder::Fn(FunctionKind::BuiltIn(BuiltInFunction {
            func: Rc::new(Arc::new(move |_, args, ctx| {
                value.clone()(&args, ctx)
            })),
            instance: Rc::new(ValueHolder::Void)
        }));

        map.insert(key.to_string(), function);
    }

    Ok(ValueHolder::Object(ObjectRef::new(map, context)))
}

#[expose]
fn input(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    let prompt = argument!(values, ValueHolder::String, "prompt", 0);
    
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_goes_into_input_above) => {},
        Err(_no_updates_is_fine) => {},
    }
    Ok(ValueHolder::String(input.trim().to_string()))
}

#[expose]
fn confirm(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
    let prompt = argument!(values, ValueHolder::String, "prompt", 0);
    
    loop {
        print!("{} [y/n]: ", prompt);
        io::stdout().flush().unwrap(); // ensure prompt shows immediately

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => return Ok(ValueHolder::Bool(true)),
            "n" | "no" => return Ok(ValueHolder::Bool(false)),
            _ => println!("Please enter 'y' or 'n'."),
        }
    }
}