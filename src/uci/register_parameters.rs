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
    type Error = Error; // Replace with your specific Error type

    fn try_from(iter: &mut Iter<'_, &str>) -> Result<Self> {
        // Peek at the first token to check for "later"
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
                    // The very next token MUST be the code
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
