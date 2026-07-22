
use nlf_macros::new_module;
use nlf_shared::{errors::LanguageResult, vm::{Array, Object, VMContext, Value}};
use serde_json::Value as JSONValue;

use crate::{errors::LanguageError, interpreter::RuntimeError};

new_module!("json", {
    fn parse(text: String, context: VMContext) -> LanguageResult<Value> {
        let object_map = convert_json_value(&serde_json::from_str(&text).map_err(|_| LanguageError::from(RuntimeError::Custom("Failed to parse JSON".into())))?, context);

        Ok(object_map)
    }

    fn stringify(value: Value, context: VMContext) -> LanguageResult<Value> {
        serde_json::to_string(&convert_value_holder(&value, context)).map(|string| Value::String(context.heap().allocate_string(string))).map_err(|_| LanguageError::from(RuntimeError::Custom("Failed to stringify JSON".into())))
    }
});

fn is_decimal(x: f64) -> bool {
    (x - x.round()).abs() > f64::EPSILON
}

fn convert_json_value(value: &JSONValue, context: VMContext) -> Value {
    match value {
        JSONValue::Bool(bool) => Value::Bool(*bool),
        JSONValue::String(string) => Value::String(context.heap().allocate_string(string.clone())),
        JSONValue::Number(number) => Value::Float(number.as_f64().unwrap()),
        JSONValue::Array(array) => {
            let array = Array {
                vec: array.iter().map(|value| convert_json_value(value, context)).collect()
            };

            Value::Array(context.heap().allocate_array(array))
        },
        JSONValue::Object(object) => {
            let object = Object {
                map: object.iter().map(|(key, value)| (key.clone(), convert_json_value(value, context))).collect()
            };

            Value::Object(context.heap().allocate_object(object))
        },
        JSONValue::Null => Value::Void
    }
}

fn convert_value_holder(value: &Value, context: VMContext) -> Option<JSONValue> {
    match value {
        Value::Bool(bool) => Some(JSONValue::Bool(*bool)),
        Value::String(string_id) => Some(JSONValue::String(context.heap().get_string(*string_id).clone())),
        Value::Float(number) => {
            Some(if is_decimal(*number) {
                JSONValue::Number(serde_json::Number::from_f64(*number).unwrap())
            } else {
                JSONValue::Number(serde_json::Number::from_u128(*number as u128).unwrap())
            })
        },
        Value::Array(array_id) => {
            let vec = context.heap().get_array(*array_id).vec.clone();
            Some(JSONValue::Array(
                vec
                    .iter()
                    .filter_map(|value| convert_value_holder(value, context))
                    .collect()
            ))
        },
        Value::Object(object_id) => {
            let map = context.heap().get_object(*object_id).map.clone();
            Some(JSONValue::Object(
                map
                    .iter()
                    .filter_map(|(key, value)| convert_value_holder(&value, context)
                    .map(|value| (key.clone(), value)))
                    .collect()
            ))
        },
        Value::Void => Some(JSONValue::Null),
        _ => None
    }
}