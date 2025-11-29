use std::{cell::RefCell, rc::Rc};

use lib::module;

use crate::{errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder, argument};

module!("math", {
    fn sqrt(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Float, "x", 0);

        Ok(ValueHolder::Float(x.sqrt()))
    }

    fn pow(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let a = argument!(values, ValueHolder::Float, "a", 0);
        let b = argument!(values, ValueHolder::Float, "b", 1);

        Ok(ValueHolder::Float(a.powf(b)))
    }

    fn abs(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Float, "x", 0);

        Ok(ValueHolder::Float(x.abs()))
    }

    fn sign(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Float, "x", 0);

        Ok(ValueHolder::Float(x.signum()))
    }

    fn sin(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Float, "x", 0);

        Ok(ValueHolder::Float(x.sin()))
    }

    fn cos(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Float, "x", 0);

        Ok(ValueHolder::Float(x.cos()))
    }

    fn tan(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Float, "x", 0);

        Ok(ValueHolder::Float(x.tan()))
    }

    fn round(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Float, "x", 0);

        Ok(ValueHolder::Float(x.round()))
    }

    fn floor(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Float, "x", 0);

        Ok(ValueHolder::Float(x.floor()))
    }

    fn ceil(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Float, "x", 0);

        Ok(ValueHolder::Float(x.ceil()))
    }
});