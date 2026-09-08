// SPDX-License-Identifier: MIT
use crate::{error::AppError, output::safe};
use std::io::{BufRead, Read, Write};
pub(crate) fn confirm(
    review: &str,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<bool, AppError> {
    writeln!(
        output,
        "{}\nType yes to continue [no]:",
        review.lines().map(safe).collect::<Vec<_>>().join("\n")
    )
    .and_then(|()| output.flush())
    .map_err(|_| AppError::new(5, "Cannot write trust prompt"))?;
    let mut line = String::new();
    input
        .take(129)
        .read_line(&mut line)
        .map_err(|_| AppError::input("Cannot read trust confirmation"))?;
    if line.len() > 128 {
        return Err(AppError::input("Trust confirmation exceeds input limit"));
    }
    match line.trim().to_ascii_lowercase().as_str() {
        "yes" => Ok(true),
        "" | "no" => Ok(false),
        _ => Err(AppError::input("Trust confirmation requires yes or no")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_yes_authorizes_and_review_is_visible_without_terminal_controls() {
        for (input, expected) in [("yes\n", true), ("no\n", false), ("", false), ("\n", false)] {
            let mut output = Vec::new();
            assert_eq!(
                confirm(
                    "Binary review\nSettings \u{1b}[31m \u{202e}",
                    &mut input.as_bytes(),
                    &mut output
                )
                .ok(),
                Some(expected)
            );
            let text = String::from_utf8_lossy(&output);
            assert!(text.contains("Binary review\nSettings"));
            assert!(!text.contains('\u{1b}'));
            assert!(!text.contains('\u{202e}'));
        }
        assert!(confirm("Review", &mut "maybe\n".as_bytes(), &mut Vec::new()).is_err());
        assert!(confirm("Review", &mut "x".repeat(129).as_bytes(), &mut Vec::new()).is_err());
    }
}
