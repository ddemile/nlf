use std::{cell::RefCell, rc::Rc, sync::{Arc}};

use lib::module;
use parking_lot::Mutex;

use crate::{argument, errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder, types::{DynamicNumber, NumberHolder}};

module!("math", {
    fn sqrt(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);
        let x: f32 = x.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(x.sqrt()))))
    }

    fn pow(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let a = argument!(values, ValueHolder::Number, "a", 0);
        let b = argument!(values, ValueHolder::Number, "b", 1);
        let a: f32 = a.into();
        let b: f32 = b.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(a.powf(b)))))
    }

    fn abs(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);
        let x: f32 = x.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(x.abs()))))
    }

    fn sign(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);
        let x: f32 = x.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(x.signum()))))
    }

    fn sin(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);
        let x: f32 = x.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(x.sin()))))
    }

    fn cos(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);
        let x: f32 = x.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(x.cos()))))
    }

    fn tan(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);
        let x: f32 = x.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(x.tan()))))
    }

    fn round(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);
        let x: f32 = x.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(x.round()))))
    }

    fn floor(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);
        let x: f32 = x.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(x.floor()))))
    }

    fn ceil(values: &[ValueHolder], _context: Rc<RefCell<ModuleContext>>) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);
        let x: f32 = x.into();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(x.ceil()))))
    }
});