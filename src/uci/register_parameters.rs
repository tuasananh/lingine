use std::slice::Iter;

use anyhow::{Error, Result};

pub enum RegisterParameters {
    Later,
    Identity {
        name: Option<String>,
        code: Option<String>,
    },
}

impl TryFrom<&mut Iter<'_, &str>> for RegisterParameters {
    type Error = Error;

    fn try_from(iter: &mut Iter<'_, &str>) -> Result<Self> {
        if let Some(&first) = iter.clone().next()
            && first == "later"
        {
            iter.next(); // Consume "later"
            return Ok(RegisterParameters::Later);
        }

        let mut name_tokens = Vec::new();
        let mut code = None;
        let mut parsing_name = false;

        while let Some(&token) = iter.next() {
            match token {
                "name" => {
                    parsing_name = true;
                }
                "code" => {
                    parsing_name = false;
                    code = iter.next().map(|s| s.to_string());
                }
                _ => {
                    if parsing_name {
                        name_tokens.push(token);
                    }
                }
            }
        }

        let name = if name_tokens.is_empty() {
            None
        } else {
            Some(name_tokens.join(" "))
        };

        Ok(RegisterParameters::Identity { name, code })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_later() {
        let tokens = "later".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        assert!(matches!(parsed, RegisterParameters::Later));
    }

    #[test]
    fn parses_later_and_leaves_remaining_tokens() {
        let tokens = "later name John".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        assert!(matches!(parsed, RegisterParameters::Later));
        assert_eq!(iter.next(), Some(&"name"));
        assert_eq!(iter.next(), Some(&"John"));
    }

    #[test]
    fn parses_empty_input() {
        let tokens = "".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name, None);
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_name_only_single_word() {
        let tokens = "name Alice".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Alice"));
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_name_only_multiple_words() {
        let tokens = "name Alice Bob Smith"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Alice Bob Smith"));
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_code_only() {
        let tokens = "code XYZ-123".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name, None);
                assert_eq!(code.as_deref(), Some("XYZ-123"));
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_name_and_code() {
        let tokens = "name Alice code XYZ-123"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Alice"));
                assert_eq!(code.as_deref(), Some("XYZ-123"));
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_code_then_name() {
        let tokens = "code XYZ-123 name Bob Smith"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Bob Smith"));
                assert_eq!(code.as_deref(), Some("XYZ-123"));
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn parses_code_without_value() {
        let tokens = "code".split_whitespace().collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name, None);
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }

    #[test]
    fn ignores_untracked_tokens_before_name() {
        let tokens = "hello world name Alice"
            .split_whitespace()
            .collect::<Vec<&str>>();
        let mut iter = tokens.iter();

        let parsed = RegisterParameters::try_from(&mut iter).unwrap();

        match parsed {
            RegisterParameters::Identity { name, code } => {
                assert_eq!(name.as_deref(), Some("Alice"));
                assert_eq!(code, None);
            }
            _ => panic!("Expected Identity variant"),
        }
    }
}
