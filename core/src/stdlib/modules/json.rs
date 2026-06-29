use std::str::FromStr;

use nlf_macros::module;
use serde_json::{Number, Value};

use crate::{argument, errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::{ArrayRef, ObjectRef, ValueHolder}};

module!("json", {
    fn parse(values: &[ValueHolder], context: &mut ModuleContext) -> RuntimeResult {
        let text = argument!(values, ValueHolder::String, "text", 0);

        let object_map = convert_json_value(&serde_json::from_str(&text).map_err(|_| LanguageError::from(RuntimeError::Custom("Failed to parse JSON".into())))?, context);

        Ok(object_map)
    }

    fn stringify(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        serde_json::to_string(&convert_value_holder(&values[0])).map(ValueHolder::String).map_err(|_| LanguageError::from(RuntimeError::Custom("Failed to stringify JSON".into())))
    }
});

impl From<ValueHolder> for Value {
    fn from(value: ValueHolder) -> Self {
        match value {
            ValueHolder::Bool(bool) => Value::Bool(bool),
            ValueHolder::String(string) => Value::String(string),
            ValueHolder::Number(number) => Value::Number(serde_json::Number::from_str(&number.to_string()).unwrap()),
            ValueHolder::Array(array) => Value::Array(array.fetch().iter().map(|value| Value::from(value.clone())).collect()),
            ValueHolder::Object(object) => Value::Object(object.fetch().iter().map(|(key, value)| (key.clone(), Value::from(value.clone()))).collect()),
            ValueHolder::Void => Value::Null,
            _ => panic!()
        }
    }
}

fn convert_json_value(value: &Value, context: &mut ModuleContext) -> ValueHolder {
    match value {
        Value::Bool(bool) => ValueHolder::Bool(*bool),
        Value::String(string) => ValueHolder::String(string.clone()),
        Value::Number(number) => ValueHolder::Number(number.as_f64().unwrap()),
        Value::Array(array) => ValueHolder::Array(ArrayRef::new(array.iter().map(|value| convert_json_value(value, context)).collect())),
        Value::Object(object) => ValueHolder::Object(ObjectRef::new(
            object.iter().map(|(key, value)| (key.clone(), convert_json_value(value, context))).collect(),
            None,
            context.get_schema_store()
        )),
        Value::Null => ValueHolder::Void
    }
}

fn convert_value_holder(value: &ValueHolder) -> Option<Value> {
    match value {
        ValueHolder::Bool(bool) => Some(Value::Bool(*bool)),
        ValueHolder::String(string) => Some(Value::String(string.clone())),
        ValueHolder::Number(number) => Some(Value::Number(Number::from_f64((*number).into()).unwrap())),
        ValueHolder::Array(array) => Some(Value::Array(array.fetch().iter().filter_map(|value| convert_value_holder(value)).collect())),
        ValueHolder::Object(object) => Some(Value::Object(object.fetch().into_iter().filter_map(|(key, value)| convert_value_holder(&value).map(|value| (key, value))).collect())),
        ValueHolder::Void => Some(Value::Null),
        _ => None
    }
}