//! Token-preserving JSON formatting with bounded scanner windows and stack.
//! The mature JSONC scanner validates spelling; this state machine validates
//! structure without constructing values or retaining the document's tokens.
use super::projection::Writer;
use jsonc_parser::{Scanner, ScannerOptions, tokens::Token};
use std::io;

#[derive(Clone, Copy, PartialEq)]
enum State {
    ArrayFirst,
    ArrayValue,
    ArrayComma,
    ObjectFirst,
    ObjectKey,
    Colon,
    Value,
    ObjectComma,
}

pub(super) fn format(
    source: &str,
    label: &str,
    out: &mut Writer<'_>,
) -> io::Result<Result<(), String>> {
    let strict = label == "JSON";
    let options = ScannerOptions {
        allow_single_quoted_strings: !strict,
        allow_hexadecimal_numbers: !strict,
        allow_unary_plus_numbers: !strict,
        allow_bare_decimal_point_numbers: !strict,
        allow_non_finite_numbers: !strict,
        allow_extended_string_escapes: !strict,
    };
    let mut stack = Vec::<State>::new();
    let mut cursor = 0;
    let mut root = false;
    let mut previous_open = false;
    let mut newline = false;
    macro_rules! invalid {
        ($reason:expr) => {
            return Ok(Err(format!("{} near source byte {}", $reason, cursor)))
        };
    }
    macro_rules! generate {
        ($text:expr) => {
            if !out.generated($text)? {
                invalid!("formatting expansion exceeds its bounded cache budget");
            }
        };
    }
    while cursor < source.len() {
        let rest = &source[cursor..];
        let trimmed = rest.trim_start_matches(|c: char| {
            (if strict {
                matches!(c, ' ' | '\t' | '\n' | '\r')
            } else {
                c.is_whitespace()
            }) || (cursor == 0 && c == '\u{feff}')
        });
        cursor += rest.len() - trimmed.len();
        if cursor == source.len() {
            break;
        }
        let mut end = (cursor + super::structured::FORMAT_BYTES).min(source.len());
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        let mut scanner = Scanner::new(&source[cursor..end], &options);
        let token = match scanner.scan() {
            Ok(Some(token)) => token,
            _ => invalid!(if end < source.len() {
                "invalid syntax or token exceeds 256 KiB"
            } else {
                "invalid JSON syntax"
            }),
        };
        if strict && scanner.token_start() != 0 {
            invalid!("invalid JSON whitespace");
        }
        let range = cursor + scanner.token_start()..cursor + scanner.token_end();
        if range.end == end
            && end < source.len()
            && !matches!(
                token,
                Token::OpenBrace
                    | Token::CloseBrace
                    | Token::OpenBracket
                    | Token::CloseBracket
                    | Token::Colon
                    | Token::Comma
            )
        {
            invalid!("a token exceeds the 256 KiB formatting budget");
        }
        if strict
            && matches!(token, Token::String(_))
            && source[range.clone()].bytes().any(|b| b < 0x20)
        {
            invalid!("unescaped control character in JSON string");
        }
        let comment = matches!(token, Token::CommentLine(_) | Token::CommentBlock(_));
        if comment {
            if strict {
                invalid!("comments are not valid JSON");
            }
            if newline {
                generate!("\n");
                generate!(&"  ".repeat(stack.len().min(64)));
            } else {
                generate!(" ");
            }
            if !out.copy(range.clone())? {
                invalid!("formatting expansion exceeds its bounded cache budget");
            }
            newline = matches!(token, Token::CommentLine(_));
            cursor = range.end;
            continue;
        }
        let close = matches!(token, Token::CloseBrace | Token::CloseBracket);
        match &token {
            Token::CloseBrace | Token::CloseBracket => {
                let valid = match (&token, stack.last()) {
                    (Token::CloseBrace, Some(State::ObjectFirst | State::ObjectComma)) => true,
                    (Token::CloseBracket, Some(State::ArrayFirst | State::ArrayComma)) => true,
                    (Token::CloseBrace, Some(State::ObjectKey))
                    | (Token::CloseBracket, Some(State::ArrayValue)) => !strict,
                    _ => false,
                };
                if !valid {
                    invalid!("unexpected closing delimiter");
                }
                stack.pop();
                if !previous_open {
                    newline = true;
                } else {
                    newline = false;
                }
            }
            Token::Comma => match stack.last_mut() {
                Some(state @ State::ArrayComma) => *state = State::ArrayValue,
                Some(state @ State::ObjectComma) => *state = State::ObjectKey,
                _ => invalid!("unexpected comma"),
            },
            Token::Colon => match stack.last_mut() {
                Some(state @ State::Colon) => *state = State::Value,
                _ => invalid!("unexpected colon"),
            },
            _ => {
                if let Some(state @ (State::ObjectFirst | State::ObjectKey)) = stack.last_mut() {
                    if !matches!(token, Token::String(_))
                        && (strict
                            || !matches!(
                                token,
                                Token::Word(_) | Token::Number(_) | Token::Boolean(_) | Token::Null
                            ))
                    {
                        invalid!("expected an object key");
                    }
                    *state = State::Colon;
                } else {
                    if matches!(token, Token::Word(_)) {
                        invalid!("unexpected JSON value");
                    }
                    match stack.last_mut() {
                        Some(state @ (State::ArrayFirst | State::ArrayValue)) => {
                            *state = State::ArrayComma
                        }
                        Some(state @ State::Value) => *state = State::ObjectComma,
                        None if !root => root = true,
                        _ => invalid!("expected a comma or end of document"),
                    }
                    if matches!(token, Token::OpenBrace | Token::OpenBracket) && stack.len() >= 128
                    {
                        invalid!("nesting exceeds the 128-level formatting budget");
                    }
                }
            }
        }
        if newline && !matches!(token, Token::Colon | Token::Comma) {
            generate!("\n");
            generate!(&"  ".repeat(stack.len().min(64)));
        }
        if !out.copy(range.clone())? {
            invalid!("formatting expansion exceeds its bounded cache budget");
        }
        previous_open = matches!(token, Token::OpenBrace | Token::OpenBracket);
        newline = previous_open || matches!(token, Token::Comma);
        if matches!(token, Token::Colon) {
            generate!(" ");
        }
        if previous_open {
            stack.push(if matches!(token, Token::OpenBrace) {
                State::ObjectFirst
            } else {
                State::ArrayFirst
            });
        }
        if close {
            previous_open = false;
        }
        cursor = range.end;
    }
    if !stack.is_empty() || !root {
        invalid!("incomplete JSON document");
    }
    if source.ends_with('\n') {
        generate!("\n");
    }
    Ok(Ok(()))
}
