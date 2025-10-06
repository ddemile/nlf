use std::collections::HashMap;

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")] // "type" field will contain the variant name
#[derive(PartialEq)]
pub enum Token {
    Keyword { value: String },
    Identifier { value: String },
    NumericLiteral { value: f64 },
    BooleanLiteral { value: bool },
    StringLiteral { value: String },
    Plus,
    Minus,
    Asterisk,
    Slash,
    Assign,
    EQ,
    NE,
    GT,
    GTE,
    LT,
    LTE,
    OpeningParenthesis,
    ClosingParenthesis,
    OpeningBracket,
    ClosingBracket,
    Comma,
    Range,
    And,
    Or
}

pub fn lex(code: String) -> Vec<Token> {
    let mut cursor = 0;

    let mut tokens: Vec<Token> = vec![];

    let char_at = |pos: usize| code.chars().nth(pos).unwrap();
    while cursor < code.len() {
        let char = char_at(cursor);
        cursor += 1;

        match char {
            ' ' | '\0' | '\n' | '\r' | '\t' => continue,
            _ => {
                let map = match_table();

                let mut longest_match = "";

                for key in map.keys() {
                    if code[(cursor - 1)..].starts_with(key) {
                        if key.len() > longest_match.len() {
                            longest_match = key;
                        }
                    }
                }

                if longest_match.len() > 0 {
                    tokens.push(map.get(longest_match).unwrap().clone());
                    cursor += longest_match.len() - 1;
                    continue;
                }

                // if let Some((key, value)) = map
                //     .iter()
                //     .find(|(key, _)| code[(cursor - 1)..].starts_with(*key))
                // {
                //     tokens.push(value.clone());
                //     cursor += key.len() - 1;
                //     continue;
                // }

                if char.is_numeric() {
                    number(&code, &mut cursor, &mut tokens);
                    continue;
                }

                if char.is_ascii_alphabetic() || char == '_' {
                    alpha(&code, &mut cursor, &mut tokens);
                    continue;
                }

                if char == '"' {
                    string(&code, &mut cursor, &mut tokens);
                    continue;
                }
            }
        };
    }

    return tokens;
}

fn match_table() -> HashMap<&'static str, Token> {
    let mut map: HashMap<&'static str, Token> = HashMap::new();

    // Punctuation
    map.insert("(", Token::OpeningParenthesis);
    map.insert(")", Token::ClosingParenthesis);
    map.insert("{", Token::OpeningBracket);
    map.insert("}", Token::ClosingBracket);
    map.insert(",", Token::Comma);
    map.insert("..", Token::Range);

    // Keywords
    map.insert("if", Token::Keyword { value: String::from("if") });
    map.insert("for", Token::Keyword { value: String::from("for") });
    map.insert("in", Token::Keyword { value: String::from("in") });
    map.insert("fn", Token::Keyword { value: String::from("fn") });

    // Booleans
    map.insert("true", Token::BooleanLiteral { value: true });
    map.insert("false", Token::BooleanLiteral { value: false });

    // Operators
    map.insert("==", Token::EQ);
    map.insert("!=", Token::NE);
    map.insert("<=", Token::LTE);
    map.insert("<", Token::LT);
    map.insert(">=", Token::GTE);
    map.insert(">", Token::GT);
    
    map.insert("=", Token::Assign);

    map.insert("+", Token::Plus);
    map.insert("-", Token::Minus);
    map.insert("*", Token::Asterisk);
    map.insert("/", Token::Slash);

    map.insert("&&", Token::And);
    map.insert("||", Token::Or);
    
    return map;
}

fn number(code: &str, cursor: &mut usize, tokens: &mut Vec<Token>) {
    let start = *cursor - 1;

    while code.chars().nth(*cursor).map_or(false, |c| c.is_numeric()) {
        *cursor += 1;
    }

    if code.chars().nth(*cursor) == Some('.') && code.chars().nth(*cursor + 1).unwrap().is_numeric() {
        *cursor += 1;
        while code.chars().nth(*cursor).unwrap().is_numeric() {
            *cursor += 1;
        }
    }

    tokens.push(Token::NumericLiteral { value: code[start..*cursor].parse().unwrap() });
}

fn alpha(code: &str, cursor: &mut usize, tokens: &mut Vec<Token>) {
    let start = *cursor - 1;

    while code.chars().nth(*cursor).map_or(false, |c| c.is_ascii_alphabetic() || c == '_' || c.is_numeric()) {
        *cursor += 1;
    }

    tokens.push(Token::Identifier { value: code[start..*cursor].to_string() });
}

fn string(code: &str, cursor: &mut usize, tokens: &mut Vec<Token>) {
    let start = *cursor;

    *cursor += 1;

    while code.chars().nth(*cursor).map_or_else(|| panic!("Unterminated string"), |c| c != '"') {
        *cursor += 1;
    }

    *cursor += 1;

    tokens.push(Token::StringLiteral { value: code[start..(*cursor - 1)].to_string() });
}