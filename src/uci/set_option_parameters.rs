use std::slice::Iter;

use anyhow::{Error, Result, ensure};

#[derive(Debug)]
pub struct SetOptionParameters {
    pub name: String,
    pub value: Option<String>,
}

impl TryFrom<&mut Iter<'_, &str>> for SetOptionParameters {
    type Error = Error;
    fn try_from(value: &mut Iter<'_, &str>) -> Result<Self> {
        let name_token = value
            .next()
            .ok_or(anyhow::anyhow!("Expect 'name', got nothing"))?;
        ensure!(name_token == &"name", "Expect 'name', got {}", name_token);
        let mut name_tokens = Vec::new();
        let mut last_token_is_value = false;
        for token in value.by_ref() {
            if token == &"value" {
                last_token_is_value = true;
                break;
            }
            name_tokens.push(*token);
        }
        ensure!(!name_tokens.is_empty(), "Option name must not be empty");
        let name = name_tokens.join(" ").to_lowercase();

        let value = if last_token_is_value {
            let collected = value
                .map(|tok| (*tok).to_string())
                .collect::<Vec<String>>()
                .join(" ");
            ensure!(!collected.is_empty(), "Option value must not be empty");
            Some(collected)
        } else {
            None
        };

        Ok(Self { name, value })
    }
}

#[cfg(test)]
mod tests {
    use super::SetOptionParameters;

    #[test]
    fn parses_single_word_name_and_value() {
        let tokens = "name Hash value 128"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = SetOptionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.name, "hash");
        assert_eq!(parsed.value.as_deref(), Some("128"));
    }

    #[test]
    fn parses_multi_word_name_and_value() {
        let tokens = "name UCI Engine About value LiNgine test build"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = SetOptionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.name, "uci engine about");
        assert_eq!(parsed.value.as_deref(), Some("LiNgine test build"));
    }

    #[test]
    fn parses_option_without_value() {
        let tokens = "name Clear Hash".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = SetOptionParameters::try_from(&mut iter).unwrap();

        assert_eq!(parsed.name, "clear hash");
        assert_eq!(parsed.value, None);
    }
}
