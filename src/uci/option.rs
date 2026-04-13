struct UCIOptionValue<T> {
    current_value: T,
    default_value: T,
}

struct UCISpinValue {
    value: UCIOptionValue<i64>,
    min: i64,
    max: i64,
}
struct UCIOptionComboValue {
    value: UCIOptionValue<String>,
    options: Vec<String>,
}

enum UCIOptionValueEnum {
    Check(UCIOptionValue<bool>),
    Spin(UCISpinValue),
    Combo(UCIOptionComboValue),
    Button,
    String(UCIOptionValue<String>),
}

pub struct UCIOption {
    name: String,
    option: UCIOptionValueEnum,
}

use std::slice::Iter;

impl From<&mut Iter<'_, &str>> for UCIOption {
    fn from(_token_stream: &mut Iter<'_, &str>) -> Self {
        Self {
            name: "Default".to_string(),
            option: UCIOptionValueEnum::Button,
        }
    }
}

pub struct UCISetOption {
    name: String,
    value: Option<String>,
}

impl From<&mut Iter<'_, &str>> for UCISetOption {
    fn from(value: &mut Iter<'_, &str>) -> Self {
        let name_token = *value.next().expect("Expect token 'name', got nothing");
        assert_eq!(
            name_token, "name",
            "Expect token 'name', got {}",
            name_token
        );

        let mut name_tokens = Vec::new();
        while let Some(token) = value.clone().next() {
            if *token == "value" {
                break;
            }
            name_tokens
                .push((*value.next().expect("Expect option name token, got nothing")).to_string());
        }
        assert!(!name_tokens.is_empty(), "Name of option not found");
        let name = name_tokens.join(" ");

        let value = if let Some(value_token) = value.clone().next() {
            if *value_token == "value" {
                value.next();
                let collected = value
                    .map(|tok| (*tok).to_string())
                    .collect::<Vec<String>>()
                    .join(" ");
                assert!(
                    !collected.is_empty(),
                    "Expect the value of option, got nothing"
                );
                Some(collected)
            } else {
                None
            }
        } else {
            None
        };

        Self { name, value }
    }
}

#[cfg(test)]
mod tests {
    use super::UCISetOption;

    #[test]
    fn parses_single_word_name_and_value() {
        let tokens = "name Hash value 128"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCISetOption::from(&mut iter);

        assert_eq!(parsed.name, "Hash");
        assert_eq!(parsed.value.as_deref(), Some("128"));
    }

    #[test]
    fn parses_multi_word_name_and_value() {
        let tokens = "name UCI Engine About value LiNgine test build"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCISetOption::from(&mut iter);

        assert_eq!(parsed.name, "UCI Engine About");
        assert_eq!(parsed.value.as_deref(), Some("LiNgine test build"));
    }

    #[test]
    fn parses_button_style_option_without_value() {
        let tokens = "name Clear Hash".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = UCISetOption::from(&mut iter);

        assert_eq!(parsed.name, "Clear Hash");
        assert_eq!(parsed.value, None);
    }
}
