use std::{cell::RefCell, rc::Rc};

use lib::module;
use rand::Rng;

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder, argument};

module!("random", {
    fn rand(_values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let mut rng = rand::rng();
        let n = rng.random();

        Ok(ValueHolder::Float(n))
    }

    fn randint(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let min = argument!(values, ValueHolder::Float, "min", 0) as i32;
        let max = argument!(values, ValueHolder::Float, "max", 1) as i32;

        let mut rng = rand::rng();
        let n = rng.random_range(min..=max);

        Ok(ValueHolder::Int(n))
    }  
});