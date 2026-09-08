// SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{cancellation::Cancellation, error::AppError, output::safe};
use permesh_provider_sdk::setup::{Input, SetupField, SetupSpec};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::{BufRead, Read, Write},
};
const MAX_INPUT_BYTES: u64 = 16_384;
fn failure() -> AppError {
    AppError::input(
        "Invalid value; use the field's indicated type and limits. Setup accepts nonsecret settings and credential references only.",
    )
}
fn answer(field: &SetupField, line: &str) -> Result<Value, AppError> {
    if line.is_empty() {
        return Ok(field.default.clone().unwrap_or(Value::Null));
    }
    match field.input {
        Input::Text { .. } | Input::Choice { .. } | Input::Credential => {
            Ok(Value::String(line.into()))
        }
        Input::Integer { .. } => line.parse::<i64>().map(Value::from).map_err(|_| failure()),
        Input::Boolean => line.parse::<bool>().map(Value::Bool).map_err(|_| failure()),
        Input::StringList { .. } | Input::Json => {
            permesh_config::parse_setup_value(line).map_err(|_| failure())
        }
    }
}
pub fn collect(
    spec: &SetupSpec,
    mut answers: BTreeMap<String, Value>,
    id: &str,
    cancel: &Cancellation,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<BTreeMap<String, Value>, AppError> {
    writeln!(output,"Permesh provider setup\n\nEnter nonsecret settings only. Credential fields take env://NAME or\nkeychain://INSTANCE/SLOT references, never passwords or tokens.\n\n{}\n{}\n",safe(&spec.title),safe(&spec.description)).map_err(|_|AppError::new(5,"Cannot write setup prompt"))?;
    let mut current_step = String::new();
    loop {
        if cancel.is_cancelled() {
            return Err(AppError::new(130, "Cancelled"));
        }
        let questions = spec
            .questions(&answers)
            .map_err(|e| AppError::input(e.to_string()))?;
        let Some(question) = questions
            .into_iter()
            .find(|q| !answers.contains_key(&q.field.key))
        else {
            break;
        };
        if current_step != question.step.id {
            writeln!(
                output,
                "{}\n{}",
                safe(&question.step.title),
                safe(&question.step.description)
            )
            .map_err(|_| AppError::new(5, "Cannot write setup prompt"))?;
            current_step = question.step.id.clone();
        }
        let field = question.field;
        writeln!(
            output,
            "\n{} ({})\n  {}",
            safe(&field.label),
            field.key,
            safe(&field.help)
        )
        .map_err(|_| AppError::new(5, "Cannot write setup prompt"))?;
        crate::setup_output::input(output, &field.input)?;
        if let Some(default) = &field.default {
            writeln!(output, "  Default: {}", safe(&default.to_string()))
                .map_err(|_| AppError::new(5, "Cannot write setup prompt"))?;
        }
        if !field.required && field.default.is_none() {
            writeln!(output, "  Optional; press Enter to skip")
                .map_err(|_| AppError::new(5, "Cannot write setup prompt"))?;
        }
        write!(output, "> ")
            .and_then(|()| output.flush())
            .map_err(|_| AppError::new(5, "Cannot write setup prompt"))?;
        let mut line = String::new();
        let count = input
            .by_ref()
            .take(MAX_INPUT_BYTES + 1)
            .read_line(&mut line)
            .map_err(|_| AppError::input("Cannot read setup answer"))?;
        if count == 0 || !line.ends_with('\n') || cancel.is_cancelled() {
            return Err(AppError::new(
                130,
                "Setup cancelled before configuration was written",
            ));
        }
        if count as u64 > MAX_INPUT_BYTES {
            return Err(AppError::input(
                "Setup answer exceeds the 16 KiB prompt limit",
            ));
        }
        let line = line.trim_end_matches(['\r', '\n']);
        let result = answer(field, line).and_then(|value| {
            let mut proposed = answers.clone();
            proposed.insert(field.key.clone(), value);
            spec.questions(&proposed).map_err(|_| failure())?;
            crate::setup::validate_references(spec, &proposed, id)?;
            Ok(proposed)
        });
        match result {
            Ok(proposed) => answers = proposed,
            Err(error) => writeln!(output, "  {}", safe(&error.message))
                .map_err(|_| AppError::new(5, "Cannot write setup prompt"))?,
        }
    }
    Ok(answers)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> SetupSpec {
        SetupSpec {
            schema_version: 1,
            title: "Example".into(),
            description: String::new(),
            steps: vec![permesh_provider_sdk::setup::SetupStep {
                id: "main".into(),
                title: "Settings".into(),
                description: String::new(),
                when: None,
                fields: vec![
                    SetupField {
                        key: "port".into(),
                        label: "Port".into(),
                        help: String::new(),
                        required: true,
                        default: Some(Value::from(443)),
                        when: None,
                        input: Input::Integer {
                            minimum: 1,
                            maximum: 65535,
                        },
                    },
                    SetupField {
                        key: "token".into(),
                        label: "Token reference".into(),
                        help: String::new(),
                        required: true,
                        default: None,
                        when: None,
                        input: Input::Credential,
                    },
                ],
            }],
        }
    }
    #[test]
    fn prompts_retry_invalid_values_and_never_echo_supplied_secrets() {
        let spec = spec();
        let mut input = std::io::Cursor::new(b"999999\n\nSENTINEL_PRIVATE\nenv://TOKEN\n");
        let mut output = vec![];
        let result = collect(
            &spec,
            BTreeMap::new(),
            "example-main",
            &Cancellation::new(),
            &mut input,
            &mut output,
        );
        assert!(result.is_ok());
        assert!(!String::from_utf8_lossy(&output).contains("SENTINEL_PRIVATE"));
        assert!(
            result.is_ok_and(|values| values["port"] == 443 && values["token"] == "env://TOKEN")
        );
    }
    #[test]
    fn eof_and_cancellation_stop_before_completion() {
        let cancel = Cancellation::new();
        let mut input = std::io::Cursor::new(b"\n");
        let mut output = vec![];
        assert!(
            collect(
                &spec(),
                BTreeMap::new(),
                "example-main",
                &cancel,
                &mut input,
                &mut output
            )
            .is_err_and(|e| e.code == 130)
        );
        cancel.cancel();
        assert!(
            collect(
                &spec(),
                BTreeMap::new(),
                "example-main",
                &cancel,
                &mut input,
                &mut output
            )
            .is_err_and(|e| e.code == 130)
        );
    }
    #[test]
    fn eof_after_a_final_unterminated_answer_cancels() {
        let mut input = std::io::Cursor::new(b"\nenv://TOKEN");
        let mut output = vec![];
        assert!(
            collect(
                &spec(),
                BTreeMap::new(),
                "example-main",
                &Cancellation::new(),
                &mut input,
                &mut output
            )
            .is_err_and(|e| e.code == 130)
        );
    }
}
