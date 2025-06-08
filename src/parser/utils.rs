//! Parser utilities

use winnow::ascii::multispace0;
use winnow::prelude::*;

/// Make a parser whitespace-insensitive
pub fn ws<'a, F, O>(mut parser: F) -> impl Parser<&'a str, O, winnow::error::ContextError>
where
    F: Parser<&'a str, O, winnow::error::ContextError>,
{
    move |input: &mut &'a str| {
        let _ = multispace0.parse_next(input)?;
        let output = parser.parse_next(input)?;
        let _ = multispace0.parse_next(input)?;
        Ok(output)
    }
}

/// Case-insensitive tag parser
#[must_use]
pub fn tag_no_case<'a>(
    tag: &'static str,
) -> impl Parser<&'a str, &'a str, winnow::error::ContextError> {
    move |input: &mut &'a str| {
        let tag_len = tag.len();
        if input.len() < tag_len {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ));
        }

        let input_start = &input[..tag_len];
        if input_start.eq_ignore_ascii_case(tag) {
            let result = input_start;
            *input = &input[tag_len..];
            Ok(result)
        } else {
            Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ))
        }
    }
}

/// Parse a delimited value with balanced delimiters
#[must_use]
pub fn balanced_delimited<'a>(
    open: char,
    close: char,
) -> impl Parser<&'a str, &'a str, winnow::error::ContextError> {
    move |input: &mut &'a str| {
        if !input.starts_with(open) {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ));
        }

        let mut depth = 0;
        let mut pos = 0;
        let bytes = input.as_bytes();

        for (i, &byte) in bytes.iter().enumerate() {
            if byte == open as u8 {
                depth += 1;
            } else if byte == close as u8 {
                depth -= 1;
                if depth == 0 {
                    pos = i + 1;
                    break;
                }
            } else if byte == b'\\' && i + 1 < bytes.len() {
                // Skip escaped character
                continue;
            }
        }

        if depth == 0 {
            let result = &input[1..pos - 1];
            *input = &input[pos..];
            Ok(result)
        } else {
            Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::default(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws() {
        let mut input = "  hello  world  ";
        let mut parser = ws("hello");
        let result = parser.parse_next(&mut input).unwrap();
        assert_eq!(result, "hello");
        assert_eq!(input, "world  ");
    }

    #[test]
    fn test_tag_no_case() {
        let mut input = "ARTICLE{...}";
        let result = tag_no_case("article").parse_next(&mut input).unwrap();
        assert_eq!(result, "ARTICLE");
        assert_eq!(input, "{...}");

        let mut input = "Article{...}";
        let result = tag_no_case("article").parse_next(&mut input).unwrap();
        assert_eq!(result, "Article");
        assert_eq!(input, "{...}");
    }
}
