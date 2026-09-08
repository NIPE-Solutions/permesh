// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{
    error::AppError,
    output::{field, safe},
};
use permesh_provider_sdk::setup::Input;
use serde_json::Value;
use std::io::{self, Write};
pub fn input(out: &mut impl Write, input: &Input) -> Result<(), AppError> {
    let result = match input {
        Input::Text {
            min_length,
            max_length,
        } => writeln!(out, "  Text: {min_length}..{max_length} characters"),
        Input::Integer { minimum, maximum } => writeln!(out, "  Integer: {minimum}..{maximum}"),
        Input::Boolean => writeln!(out, "  Boolean: true or false"),
        Input::StringList {
            min_items,
            max_items,
        } => writeln!(
            out,
            "  YAML/JSON string list: {min_items}..{max_items} items"
        ),
        Input::Json => writeln!(out, "  Structured YAML/JSON value (single line)"),
        Input::Credential => writeln!(
            out,
            "  Credential reference only: env://NAME or keychain://INSTANCE/SLOT"
        ),
        Input::Choice { options } => options.iter().try_for_each(|choice| {
            writeln!(out, "  {}: {}", safe(&choice.value), safe(&choice.label))
        }),
    };
    result.map_err(|_| AppError::new(5, "Cannot write setup prompt"))
}
pub fn write(out: &mut impl Write, result: &Value) -> io::Result<()> {
    writeln!(
        out,
        "{}\n  Provider: {}\n  SHA-256: {}\n",
        safe(field(&result["spec"], "title")),
        safe(field(result, "provider")),
        safe(field(result, "sha256"))
    )?;
    writeln!(
        out,
        "Use --json for the complete schema, conditions, defaults and validation limits.\n"
    )?;
    if let Some(steps) = result["spec"]["steps"].as_array() {
        for step in steps {
            writeln!(
                out,
                "{}{}",
                safe(field(step, "title")),
                if step["when"].is_object() {
                    " (conditional)"
                } else {
                    ""
                }
            )?;
            if let Some(fields) = step["fields"].as_array() {
                for item in fields {
                    writeln!(
                        out,
                        "  {}: {} ({})",
                        safe(field(item, "key")),
                        safe(field(item, "label")),
                        safe(field(&item["input"], "type"))
                    )?;
                }
            }
            writeln!(out)?;
        }
    }
    Ok(())
}
