struct UCIOptionValue<T> {
    current_value: T,
    default_value: T,
}

struct UCISpinValue {
    value: UCIOptionValue<i64>,
    min: i64,
    max: i64
}
struct UCIOptionComboValue {
    value: UCIOptionValue<String>,
    options: Vec<String>
}

enum UCIOptionValueEnum {
    Check(UCIOptionValue<bool>),
    Spin(UCISpinValue),
    Combo(UCIOptionComboValue),
    Button,
    String(UCIOptionValue<String>)
}

pub struct UCIOption {
    name: String,
    option: UCIOptionValueEnum
}

use std::slice::Iter;

impl From<&mut Iter<'_, &str>> for UCIOption { 
    fn from(_token_stream: &mut Iter<'_, &str>) -> Self {
        Self {
            name: "Default".to_string(),
            option: UCIOptionValueEnum::Button
        } 
    }
}

pub struct UCISetOption {
    name: String,
    value: Option<String>
}

impl From<&mut Iter<'_, &str>> for UCISetOption {
    fn from(value: &mut Iter<'_, &str>) -> Self {
        let name_token = *value.next().expect("Expect token 'name', got nothing");
        assert_eq!(name_token, "name", "Expect token 'name', got {}", name_token);
        let name = value.next().expect("Name of option not found").to_string();
        let value = if let Some(value_token) = value.next() {
            assert_eq!(*value_token, "value", "Expect token 'value', got {}", *value_token);
            let v = value.next().expect("Expect the value of option, got nothing").to_string();
            Some(v)
        } else {
            None
        };

        Self {
            name, 
            value
        }
    }
}