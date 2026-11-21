use std::f64::consts::PI;

use nlf_macros::module;
use nlf_shared::{errors::LanguageResult, vm::{VMContext, Value}};

module!("math", {
    fn sqrt(x: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(x.sqrt()))
    }

    fn pow(a: f64, b: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(a.powf(b)))
    }

    fn abs(x: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(x.abs()))
    }

    fn sign(x: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(x.signum()))
    }

    fn sin(x: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(x.sin()))
    }

    fn cos(x: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(x.cos()))
    }

    fn tan(x: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(x.tan()))
    }

    fn round(x: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(x.round()))
    }

    fn floor(x: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(x.floor()))
    }

    fn ceil(x: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(x.ceil()))
    }

    fn pi(_: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(PI))
    }

    fn min(a: f64, b: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(a.min(b)))
    }

    fn max(a: f64, b: f64, _: VMContext) -> LanguageResult<Value> {
        Ok(Value::Float(a.max(b)))
    }
});