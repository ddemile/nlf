use crate::parser::{ArrayRef, ObjectRef, ValueHolder};
use inline_colorization::*;

#[derive(Default)]
pub struct FormatOptions {
    pub space: Option<usize>
}

pub fn format_object(object_ref: &ObjectRef, options: FormatOptions) -> String {
    let object = object_ref.fetch();

    let mut string = String::new();

    string.push_str("{");

    if options.space.is_some()  {
        string.push_str("\n");
    }

    let spaces = if let Some(space) = options.space {
        " ".repeat(space)
    } else {
        " ".to_string()
    };

    for (index, (key, value)) in object.iter().enumerate() {
        string.push_str(&spaces);
        string.push_str(&format_string(key));
        string.push_str(": ");

        let mut formatted_value = match value {
            ValueHolder::String(string) => format_string(string),
            value => format!("{value}")
        };

        if formatted_value.lines().count() > 1 {
            let lines: Vec<String> = formatted_value.lines().enumerate().map(|(index, line)| format!("{}{line}", if index > 0 { &spaces } else { "" })).collect();
            formatted_value = lines.join("\n");
        }

        string.push_str(&format!("{formatted_value}"));

        if index != object.len() - 1 {
            string.push(',');
        }

        if options.space.is_none() {
            string.push(' ');
        }

        if options.space.is_some() {
            string.push('\n');
        }
    }
    string.push('}');

    string
}

pub fn format_array(array_ref: &ArrayRef, options: FormatOptions) -> String {
    let array = array_ref.fetch();

    let mut string = String::new();

    string.push_str("[");

    if options.space.is_some()  {
        string.push_str("\n");
    }

    let spaces = if let Some(space) = options.space {
        " ".repeat(space)
    } else {
        " ".to_string()
    };

    for (index, value) in array.iter().enumerate() {
        string.push_str(&spaces);

        let mut formatted_value = match value {
            ValueHolder::String(string) => format_string(string),
            value => format!("{value}")
        };

        if formatted_value.lines().count() > 1 {
            let lines: Vec<String> = formatted_value.lines().enumerate().map(|(index, line)| format!("{}{line}", if index > 0 { &spaces } else { "" })).collect();
            formatted_value = lines.join("\n");
        }

        string.push_str(&format!("{formatted_value}"));

        if index != array.len() - 1 {
            string.push(',');
        }

        if options.space.is_none() {
            string.push(' ');
        }

        if options.space.is_some() {
            string.push('\n');
        }
    }
    string.push(']');

    string
}

pub fn format_string(string: &str) -> String {
    format!("{color_green}\"{string}\"{color_reset}")
}