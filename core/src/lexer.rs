use std::{collections::HashMap, f32::consts::PI};

use serde::{Serialize, Deserialize};

use crate::errors::{LanguageError, LanguageResult, LexerError};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[derive(PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")] // "type" field will contain the variant name
#[derive(PartialEq)]
pub enum TokenKind {
    Keyword(KeywordKind),
    Identifier { value: String },
    NumericLiteral { value: String },
    BooleanLiteral { value: bool },
    StringLiteral { value: String },
    Plus,
    Minus,
    Asterisk,
    Slash,
    Percent,
    Assign,
    PlusEqual,
    MinusEqual,
    AsteriskEqual,
    SlashEqual,
    PercentEqual,
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
    OpeningSquareBracket,
    ClosingSquareBracket,
    Comma,
    Range,
    And,
    Or,
    Period,
    Colon,
    Arrow,
    Bang
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "kind")] // "kind" field will contain the variant name
#[derive(PartialEq)]
pub enum KeywordKind {
    If,
    Else,
    For,
    While,
    In,
    Fn,
    Return,
    Break,
    Throw,
    Try,
    Catch,
    Let,
    Export,
    Import,
    From,
    Class,
    Public,
    Protected,
    Private
}

pub fn lex(code: String) -> LanguageResult<Vec<Token>> {
    let mut cursor = 0;

    let mut tokens: Vec<Token> = vec![];

    let char_at = |pos: usize| code.chars().nth(pos);
    while cursor < code.len() {
        let (char_byte_len, char) = match code[cursor..].chars().next() {
            Some(c) => (c.len_utf8(), c),
            None => break,
        };

        match char {
            ' ' | '\0' | '\n' | '\r' | '\t' => {
                cursor += char_byte_len;
                continue;
            },
            _ => {
                let map = match_table();

                if code[cursor..].starts_with("//") {
                    while !matches!(char_at(cursor), Some('\n')) && cursor < code.len() {
                        cursor += 1;
                    }
                    cursor += 1;
                }

                let mut longest_match = "";

                for key in map.keys() {
                    if code[cursor..].starts_with(key) {
                        if key.len() > longest_match.len() {
                            longest_match = key;
                        }
                    }
                }

                if longest_match.len() > 0 {
                    let kind = map.get(longest_match).unwrap().clone();

                    let next_char = code[cursor + longest_match.len()..].chars().next();

                    if !matches!(kind, TokenKind::Keyword(_)) || (matches!(kind, TokenKind::Keyword(_)) && matches!(next_char.map(|char| char.is_ascii_alphabetic() || char == '_'), Some(false))) {
                        tokens.push(Token { kind, start: cursor, end: cursor + longest_match.len() });
                        cursor += longest_match.len();
                        continue;
                    }
                }

                if char.is_numeric() {
                    number(&code, &mut cursor, &mut tokens);
                    continue;
                }

                if char.is_ascii_alphabetic() || char == '_' {
                    alpha(&code, &mut cursor, &mut tokens);
                    continue;
                }

                if char == '"' {
                    string(&code, &mut cursor, &mut tokens)?;
                    continue;
                }

                cursor += char_byte_len;
            }
        };
    }

    Ok(tokens)
}

pub fn match_table() -> HashMap<&'static str, TokenKind> {
    let mut map: HashMap<&'static str, TokenKind> = HashMap::new();

    // Punctuation
    map.insert("(", TokenKind::OpeningParenthesis);
    map.insert(")", TokenKind::ClosingParenthesis);
    map.insert("{", TokenKind::OpeningBracket);
    map.insert("}", TokenKind::ClosingBracket);
    map.insert("[", TokenKind::OpeningSquareBracket);
    map.insert("]", TokenKind::ClosingSquareBracket);
    map.insert(",", TokenKind::Comma);
    map.insert("..", TokenKind::Range);
    map.insert(".", TokenKind::Period);
    map.insert(":", TokenKind::Colon);

    // Keywords
    map.insert("if", TokenKind::Keyword(KeywordKind::If));
    map.insert("else", TokenKind::Keyword(KeywordKind::Else));
    map.insert("for", TokenKind::Keyword(KeywordKind::For));
    map.insert("while", TokenKind::Keyword(KeywordKind::While));
    map.insert("in", TokenKind::Keyword(KeywordKind::In));
    map.insert("fn", TokenKind::Keyword(KeywordKind::Fn));
    map.insert("return", TokenKind::Keyword(KeywordKind::Return));
    map.insert("break", TokenKind::Keyword(KeywordKind::Break));
    map.insert("throw", TokenKind::Keyword(KeywordKind::Throw));
    map.insert("try", TokenKind::Keyword(KeywordKind::Try));
    map.insert("catch", TokenKind::Keyword(KeywordKind::Catch));
    map.insert("let", TokenKind::Keyword(KeywordKind::Let));
    map.insert("export", TokenKind::Keyword(KeywordKind::Export));
    map.insert("import", TokenKind::Keyword(KeywordKind::Import));
    map.insert("from", TokenKind::Keyword(KeywordKind::From));
    map.insert("class", TokenKind::Keyword(KeywordKind::Class));
    map.insert("public", TokenKind::Keyword(KeywordKind::Public));
    map.insert("protected", TokenKind::Keyword(KeywordKind::Protected));
    map.insert("private", TokenKind::Keyword(KeywordKind::Private));

    // Booleans
    map.insert("true", TokenKind::BooleanLiteral { value: true });
    map.insert("false", TokenKind::BooleanLiteral { value: false });

    // Operators
    map.insert("==", TokenKind::EQ);
    map.insert("!=", TokenKind::NE);
    map.insert("<=", TokenKind::LTE);
    map.insert("<", TokenKind::LT);
    map.insert(">=", TokenKind::GTE);
    map.insert(">", TokenKind::GT);
    
    map.insert("=", TokenKind::Assign);

    map.insert("+", TokenKind::Plus);
    map.insert("-", TokenKind::Minus);
    map.insert("*", TokenKind::Asterisk);
    map.insert("/", TokenKind::Slash);
    map.insert("%", TokenKind::Percent);
    map.insert("!", TokenKind::Bang);

    map.insert("+=", TokenKind::PlusEqual);
    map.insert("-=", TokenKind::MinusEqual);
    map.insert("*=", TokenKind::AsteriskEqual);
    map.insert("/=", TokenKind::SlashEqual);
    map.insert("%=", TokenKind::PercentEqual);

    map.insert("&&", TokenKind::And);
    map.insert("||", TokenKind::Or);

    map.insert("=>", TokenKind::Arrow);

    map.insert("π", TokenKind::NumericLiteral { value: PI.to_string() });

    return map;
}

fn number(code: &str, cursor: &mut usize, tokens: &mut Vec<Token>) {
    let start = *cursor;

    while let Some(c) = code[*cursor..].chars().next() {
        if c.is_numeric() {
            *cursor += c.len_utf8();
        } else {
            break;
        }
    }

    if let Some('.') = code[*cursor..].chars().next() {
        let mut iter = code[*cursor..].chars();
        iter.next(); // skip '.'
        if let Some(next_digit) = iter.next() {
            if next_digit.is_numeric() {
                *cursor += 1; // dot is 1 byte
                *cursor += next_digit.len_utf8();
                while let Some(c) = code[*cursor..].chars().next() {
                    if c.is_numeric() {
                        *cursor += c.len_utf8();
                    } else {
                        break;
                    }
                }
            }
        }
    }

    tokens.push(Token { kind: TokenKind::NumericLiteral { value: code[start..*cursor].to_string() }, start, end: *cursor });
}

fn alpha(code: &str, cursor: &mut usize, tokens: &mut Vec<Token>) {
    let start = *cursor;

    while let Some(c) = code[*cursor..].chars().next() {
        if c.is_ascii_alphabetic() || c == '_' || c.is_numeric() {
            *cursor += c.len_utf8();
        } else {
            break;
        }
    }

    tokens.push(Token { kind: TokenKind::Identifier { value: code[start..*cursor].to_string() }, start, end: *cursor });
}

fn string(code: &str, cursor: &mut usize, tokens: &mut Vec<Token>) -> LanguageResult<()> {
    let start = *cursor; // points at opening quote
    *cursor += 1; // skip the opening quote

    let string_start = *cursor; // start of actual string content

    let mut iter = code[*cursor..].char_indices();

    while let Some((i, c)) = iter.next() {
        if c == '"' {
            // Found closing quote
            *cursor = string_start + i + c.len_utf8();

            tokens.push(Token {
                kind: TokenKind::StringLiteral {
                    value: code[string_start..string_start + i].to_string(), // inner string only
                },
                start: start,
                end: *cursor,
            });

            return Ok(());
        }
    }

    // Unterminated string: advance cursor to end
    *cursor = code.len();

    Err(LanguageError::with_source(
        LexerError::UnterminatedString,
        start,
        *cursor,
    ))
}