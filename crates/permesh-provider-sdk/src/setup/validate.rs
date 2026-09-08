// SPDX-License-Identifier: MIT OR Apache-2.0
use super::{Condition, Input, MAX_SETUP_BYTES, SetupError, SetupField, SetupSpec};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
};

impl SetupSpec {
    pub fn validate(&self) -> Result<(), SetupError> {
        if self.schema_version != 1 {
            return Err(SetupError::Version);
        }
        ui(&self.title, 128, true)?;
        ui(&self.description, 1024, false)?;
        if self.steps.is_empty() || self.steps.len() > 32 {
            return Err(SetupError::Limit);
        }
        let mut steps = BTreeSet::new();
        let mut fields = BTreeMap::new();
        let mut credentials = 0;
        for step in &self.steps {
            if !identifier(&step.id) || !steps.insert(&step.id) {
                return Err(SetupError::Schema);
            }
            ui(&step.title, 128, true)?;
            ui(&step.description, 1024, false)?;
            condition(step.when.as_ref(), &fields)?;
            if step.fields.is_empty() {
                return Err(SetupError::Schema);
            }
            for field in &step.fields {
                if fields.len() == 128 {
                    return Err(SetupError::Limit);
                }
                if !identifier(&field.key) || fields.contains_key(field.key.as_str()) {
                    return Err(SetupError::Schema);
                }
                ui(&field.label, 128, true)?;
                ui(&field.help, 1024, false)?;
                input(&field.input)?;
                condition(field.when.as_ref(), &fields)?;
                if matches!(field.input, Input::Credential) {
                    credentials += 1;
                }
                if credentials > 16 {
                    return Err(SetupError::Limit);
                }
                if let Some(default) = &field.default {
                    if default.is_null() || matches!(field.input, Input::Credential) {
                        return Err(SetupError::Schema);
                    }
                    value(default)?;
                    if !accepts(&field.input, default) {
                        return Err(SetupError::Schema);
                    }
                }
                fields.insert(field.key.as_str(), field);
            }
        }
        bounded(self)
    }
}
fn identifier(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
fn ui(value: &str, max: usize, required: bool) -> Result<(), SetupError> {
    if (required && value.trim().is_empty()) || value.chars().count() > max
        || value.chars().any(|c| c.is_control() || matches!(c,'\u{061c}'|'\u{200e}'|'\u{200f}'|'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')) {
        return Err(SetupError::Schema);
    }
    Ok(())
}
fn input(input: &Input) -> Result<(), SetupError> {
    match input {
        Input::Text {
            min_length,
            max_length,
        } if min_length > max_length || *max_length > 4096 => Err(SetupError::Schema),
        Input::Integer { minimum, maximum } if minimum > maximum => Err(SetupError::Schema),
        Input::StringList {
            min_items,
            max_items,
        } if min_items > max_items || *max_items > 128 => Err(SetupError::Schema),
        Input::Choice { options } => {
            if options.is_empty() || options.len() > 128 {
                return Err(SetupError::Limit);
            }
            let mut values = BTreeSet::new();
            for option in options {
                ui(&option.label, 128, true)?;
                ui(&option.value, MAX_SETUP_BYTES, false)?;
                if !values.insert(&option.value) {
                    return Err(SetupError::Schema);
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn condition(
    when: Option<&Condition>,
    fields: &BTreeMap<&str, &SetupField>,
) -> Result<(), SetupError> {
    let Some(when) = when else {
        return Ok(());
    };
    let field = fields
        .get(when.field.as_str())
        .ok_or(SetupError::Condition)?;
    if !matches!(
        field.input,
        Input::Text { .. } | Input::Integer { .. } | Input::Boolean | Input::Choice { .. }
    ) || !accepts(&field.input, &when.equals)
    {
        return Err(SetupError::Condition);
    }
    value(&when.equals)
}
pub(super) fn accepts(input: &Input, value: &Value) -> bool {
    match input {
        Input::Text {
            min_length,
            max_length,
        } => value
            .as_str()
            .is_some_and(|v| (*min_length..=*max_length).contains(&v.chars().count())),
        Input::Integer { minimum, maximum } => value
            .as_i64()
            .is_some_and(|v| (*minimum..=*maximum).contains(&v)),
        Input::Boolean => value.is_boolean(),
        Input::Choice { options } => value
            .as_str()
            .is_some_and(|v| options.iter().any(|o| o.value == v)),
        Input::StringList {
            min_items,
            max_items,
        } => value.as_array().is_some_and(|items| {
            (*min_items..=*max_items).contains(&items.len())
                && items
                    .iter()
                    .all(|v| v.as_str().is_some_and(|v| v.chars().count() <= 4096))
        }),
        Input::Json => true,
        Input::Credential => value
            .as_str()
            .is_some_and(|v| !v.is_empty() && v.len() <= MAX_SETUP_BYTES),
    }
}
pub(super) fn depth(value: &Value, level: usize) -> bool {
    if level > 16 {
        return false;
    }
    match value {
        Value::Array(items) => items.iter().all(|v| depth(v, level + 1)),
        Value::Object(items) => items.values().all(|v| depth(v, level + 1)),
        _ => true,
    }
}
pub(super) fn value(value: &Value) -> Result<(), SetupError> {
    if !depth(value, 1) {
        return Err(SetupError::Limit);
    }
    bounded(value)
}
struct Counter(usize);
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.0 {
            return Err(io::Error::other("setup exceeds limit"));
        }
        self.0 -= bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn bounded(value: &impl Serialize) -> Result<(), SetupError> {
    serde_json::to_writer(Counter(MAX_SETUP_BYTES), value).map_err(|_| SetupError::Limit)
}
