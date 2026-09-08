// SPDX-License-Identifier: MIT OR Apache-2.0
use super::{Condition, Input, Question, ResolvedSetup, SetupError, SetupSpec, validate};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

impl SetupSpec {
    /// Return all active fields in declaration order, allowing required fields
    /// to remain unanswered. Supplied answers for inactive fields are errors.
    pub fn questions(
        &self,
        answers: &BTreeMap<String, Value>,
    ) -> Result<Vec<Question<'_>>, SetupError> {
        self.evaluate(answers, false)
            .map(|(questions, _)| questions)
    }
    /// Apply defaults, omit optional nulls, and require every active required
    /// field. Credential values are strings; the CLI validates secret references.
    pub fn resolve(&self, answers: &BTreeMap<String, Value>) -> Result<ResolvedSetup, SetupError> {
        self.evaluate(answers, true).map(|(_, resolved)| resolved)
    }
    fn evaluate(
        &self,
        answers: &BTreeMap<String, Value>,
        require: bool,
    ) -> Result<(Vec<Question<'_>>, ResolvedSetup), SetupError> {
        self.validate()?;
        let keys: BTreeSet<_> = self
            .steps
            .iter()
            .flat_map(|s| s.fields.iter().map(|f| f.key.as_str()))
            .collect();
        for (key, value) in answers {
            if !keys.contains(key.as_str()) {
                return Err(SetupError::UnknownAnswer);
            }
            validate::value(value)?;
        }
        validate::bounded(answers)?;
        let mut questions = Vec::new();
        let mut effective = BTreeMap::new();
        let mut resolved = ResolvedSetup::default();
        for step in &self.steps {
            let step_active = active(step.when.as_ref(), &effective);
            for field in &step.fields {
                let field_active = step_active && active(field.when.as_ref(), &effective);
                if !field_active {
                    if answers.contains_key(&field.key) {
                        return Err(SetupError::InactiveAnswer);
                    }
                    continue;
                }
                questions.push(Question { step, field });
                let value = answers.get(&field.key).or(field.default.as_ref());
                let Some(value) = value.filter(|value| !value.is_null()) else {
                    if field.required && (require || answers.contains_key(&field.key)) {
                        return Err(SetupError::Required);
                    }
                    continue;
                };
                if !validate::accepts(&field.input, value) {
                    return Err(SetupError::Answer);
                }
                effective.insert(field.key.as_str(), value);
                if matches!(field.input, Input::Credential) {
                    let reference = value.as_str().ok_or(SetupError::Answer)?;
                    resolved
                        .credentials
                        .insert(field.key.clone(), reference.to_owned());
                } else {
                    resolved
                        .configuration
                        .insert(field.key.clone(), value.clone());
                }
            }
        }
        validate::bounded(&resolved.configuration)?;
        validate::bounded(&resolved.credentials)?;
        // Applying many individually valid defaults cannot exceed the total
        // normalized answer budget even when the caller supplied no values.
        validate::bounded(&effective)?;
        Ok((questions, resolved))
    }
}
fn active(condition: Option<&Condition>, effective: &BTreeMap<&str, &Value>) -> bool {
    condition.is_none_or(|condition| {
        effective
            .get(condition.field.as_str())
            .is_some_and(|value| **value == condition.equals)
    })
}
