use std::{cell::RefCell, rc::Rc};

use lib::module;
use rand::Rng;

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder};

module!("random", {
    fn rand(_values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let mut rng = rand::rng();
        let n = rng.random();

        Ok(ValueHolder::Float(n))
    }

    fn randint(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let Some(ValueHolder::Float(min)) = values.get(0) else {
            return Err(LanguageError::from(
                RuntimeError::Custom("Missing argument 'min'".into())
            ));
        };

        let Some(ValueHolder::Float(max)) = values.get(1) else {
            return Err(LanguageError::from(
                RuntimeError::Custom("Missing argument 'max'".into())
            ));
        };

        let mut rng = rand::rng();
        let n = rng.random_range(*min as i32..=*max as i32);

        Ok(ValueHolder::Int(n))
    }  
});