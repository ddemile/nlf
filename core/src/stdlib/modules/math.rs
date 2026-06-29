use std::f64::consts::PI;

use nlf_macros::module;

use crate::{argument, errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder};

module!("math", {
    fn sqrt(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);

        Ok(ValueHolder::Number(x.sqrt()))
    }

    fn pow(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let a = argument!(values, ValueHolder::Number, "a", 0);
        let b = argument!(values, ValueHolder::Number, "b", 1);

        Ok(ValueHolder::Number(a.powf(b)))
    }

    fn abs(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);

        Ok(ValueHolder::Number(x.abs()))
    }

    fn sign(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);

        Ok(ValueHolder::Number(x.signum()))
    }

    fn sin(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);

        Ok(ValueHolder::Number(x.sin()))
    }

    fn cos(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);

        Ok(ValueHolder::Number(x.cos()))
    }

    fn tan(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);

        Ok(ValueHolder::Number(x.tan()))
    }

    fn round(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);

        Ok(ValueHolder::Number(x.round()))
    }

    fn floor(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);

        Ok(ValueHolder::Number(x.floor()))
    }

    fn ceil(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let x = argument!(values, ValueHolder::Number, "x", 0);

        Ok(ValueHolder::Number(x.ceil()))
    }

    fn pi(_values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        Ok(ValueHolder::Number(PI))
    }

    fn min(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let a = argument!(values, ValueHolder::Number, "a", 0);
        let b = argument!(values, ValueHolder::Number, "b", 1);

        Ok(ValueHolder::Number(a.min(b)))
    }

    fn max(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let a = argument!(values, ValueHolder::Number, "a", 0);
        let b = argument!(values, ValueHolder::Number, "b", 1);

        Ok(ValueHolder::Number(a.max(b)))
    }
});