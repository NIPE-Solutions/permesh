// SPDX-License-Identifier: MIT
#![allow(clippy::unwrap_used)]
use permesh_provider_sdk::setup::{Input, SetupSpec};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[test]
fn published_setup_spec_validates_and_selects_authentication_questions() {
    let schema: SetupSpec =
        serde_json::from_str(include_str!("../../../examples/setup/spec.json")).unwrap();
    schema.validate().unwrap();
    let keys = |answers: &BTreeMap<String, Value>| {
        schema
            .questions(answers)
            .unwrap()
            .into_iter()
            .map(|question| question.field.key.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        keys(&BTreeMap::new()),
        [
            "auth_method",
            "token",
            "tenant",
            "port",
            "enabled",
            "regions",
            "metadata"
        ]
    );
    assert_eq!(
        keys(&BTreeMap::from([("auth_method".into(), json!("service"))])),
        [
            "auth_method",
            "client_id",
            "client_secret",
            "tenant",
            "port",
            "enabled",
            "regions",
            "metadata"
        ]
    );
}

fn spec_value() -> Value {
    json!({"schema_version":1,"title":"Connect provider","description":"Settings and credential references","steps":[
     {"id":"connection","title":"Connection","description":"", "fields":[
      {"key":"auth","label":"Authentication","help":"","required":true,"default":"token","input":{"type":"choice","options":[{"value":"token","label":"Token"},{"value":"service","label":"Service"}]}},
      {"key":"token","label":"Token reference","help":"","required":true,"when":{"field":"auth","equals":"token"},"input":{"type":"credential"}},
      {"key":"tenant","label":"Tenant","help":"","required":true,"input":{"type":"text","min_length":1,"max_length":32}},
      {"key":"port","label":"Port","help":"","required":true,"default":443,"input":{"type":"integer","minimum":1,"maximum":65535}},
      {"key":"enabled","label":"Enabled","help":"","required":false,"default":true,"input":{"type":"boolean"}},
      {"key":"regions","label":"Regions","help":"","required":false,"input":{"type":"string_list","min_items":0,"max_items":3}},
      {"key":"metadata","label":"Metadata","help":"","required":false,"input":{"type":"json"}}
     ]},
     {"id":"service","title":"Service","description":"","when":{"field":"auth","equals":"service"},"fields":[
      {"key":"client_id","label":"Client ID","help":"","required":true,"input":{"type":"text","min_length":1,"max_length":64}},
      {"key":"client_secret","label":"Secret reference","help":"","required":true,"input":{"type":"credential"}}
     ]}
    ]})
}
fn spec() -> SetupSpec {
    serde_json::from_value(spec_value()).unwrap()
}
fn answers() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("tenant".into(), json!("acme")),
        ("token".into(), json!("env://ACME_TOKEN")),
    ])
}
#[test]
fn defaults_conditions_all_types_and_order() {
    let spec = spec();
    spec.validate().unwrap();
    let keys: Vec<_> = spec
        .questions(&BTreeMap::new())
        .unwrap()
        .iter()
        .map(|q| q.field.key.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "auth", "token", "tenant", "port", "enabled", "regions", "metadata"
        ]
    );
    let mut answers = answers();
    answers.insert("regions".into(), json!(["eu", "us"]));
    answers.insert("metadata".into(), json!({"nested":[1,true]}));
    let resolved = spec.resolve(&answers).unwrap();
    assert_eq!(resolved.configuration["port"], json!(443));
    assert_eq!(resolved.configuration["enabled"], json!(true));
    assert_eq!(resolved.configuration["regions"], json!(["eu", "us"]));
    assert_eq!(
        resolved.configuration["metadata"],
        json!({"nested":[1,true]})
    );
    assert_eq!(
        resolved.credentials,
        BTreeMap::from([("token".into(), "env://ACME_TOKEN".into())])
    );
    assert!(!resolved.configuration.contains_key("token"));
    answers.remove("token");
    answers.insert("auth".into(), json!("service"));
    answers.insert("client_id".into(), json!("app"));
    answers.insert(
        "client_secret".into(),
        json!("keychain://main/client_secret"),
    );
    let resolved = spec.resolve(&answers).unwrap();
    assert!(!resolved.credentials.contains_key("token"));
    assert!(resolved.credentials.contains_key("client_secret"));
}
#[test]
fn partial_questions_allow_missing_required_but_resolve_does_not() {
    let spec = spec();
    assert!(spec.questions(&BTreeMap::new()).is_ok());
    assert!(spec.resolve(&BTreeMap::new()).is_err());
    let mut answers = answers();
    answers.insert("enabled".into(), Value::Null);
    assert!(
        !spec
            .resolve(&answers)
            .unwrap()
            .configuration
            .contains_key("enabled")
    );
    answers.insert("port".into(), Value::Null);
    assert!(spec.questions(&answers).is_err());
}
#[test]
fn rejects_unknown_inactive_and_invalid_answers_without_echo() {
    for (key, value) in [
        ("extra", json!("SENTINEL")),
        ("client_id", json!("SENTINEL")),
        ("port", json!("SENTINEL")),
        ("enabled", json!(1)),
        ("regions", json!([1])),
        ("tenant", json!("")),
        ("token", json!(false)),
    ] {
        let mut answers = answers();
        answers.insert(key.into(), value);
        let error = spec().questions(&answers).unwrap_err();
        assert!(!format!("{error:?} {error}").contains("SENTINEL"));
    }
}
#[test]
fn strict_schema_roundtrip_and_no_defaults_for_credentials() {
    let spec = spec();
    assert_eq!(serde_json::to_value(&spec).unwrap(), spec_value());
    for path in ["spec", "step", "field", "input", "choice", "condition"] {
        let mut value = spec_value();
        let object = match path {
            "spec" => &mut value,
            "step" => &mut value["steps"][0],
            "field" => &mut value["steps"][0]["fields"][0],
            "input" => &mut value["steps"][0]["fields"][0]["input"],
            "choice" => &mut value["steps"][0]["fields"][0]["input"]["options"][0],
            _ => &mut value["steps"][0]["fields"][1]["when"],
        };
        object["unknown"] = json!(true);
        assert!(serde_json::from_value::<SetupSpec>(value).is_err());
    }
    let mut value = spec_value();
    value["steps"][0]["fields"][1]["default"] = json!("SENTINEL");
    assert!(
        serde_json::from_value::<SetupSpec>(value)
            .unwrap()
            .validate()
            .is_err()
    );
}
#[test]
fn condition_references_must_be_earlier_scalar_and_typed() {
    for (field, equals) in [
        ("tenant", json!("acme")),
        ("token", json!("x")),
        ("auth", json!(7)),
        ("missing", json!(true)),
    ] {
        let mut value = spec_value();
        value["steps"][0]["fields"][1]["when"] = json!({"field":field,"equals":equals});
        assert!(
            serde_json::from_value::<SetupSpec>(value)
                .unwrap()
                .validate()
                .is_err()
        );
    }
}
#[test]
fn question_exposes_borrowed_step_and_input() {
    let spec = spec();
    let questions = spec.questions(&BTreeMap::new()).unwrap();
    assert_eq!(questions[0].step.id, "connection");
    assert!(matches!(questions[0].field.input, Input::Choice { .. }));
}

#[test]
fn rejects_hostile_labels_ids_bounds_and_duplicate_declarations() {
    for (pointer, value) in [
        ("/schema_version", json!(2)),
        ("/title", json!("\u{1b}[31mSENTINEL")),
        ("/description", json!("\u{202e}hidden")),
        ("/steps/0/title", json!(" ")),
        ("/steps/0/id", json!("../escape")),
        ("/steps/0/fields/0/key", json!("1bad")),
        ("/steps/0/fields/0/label", json!("\u{2066}hidden")),
        ("/steps/0/fields/0/help", json!("\nSENTINEL")),
        ("/steps/0/fields/0/default", Value::Null),
        ("/steps/0/fields/0/input/options/1/value", json!("token")),
        (
            "/steps/0/fields/0/input/options/0/value",
            json!("\u{1b}[31m"),
        ),
        ("/steps/0/fields/0/input/options/0/label", json!("")),
        ("/steps/0/fields/2/input/max_length", json!(4097)),
        ("/steps/0/fields/2/input/min_length", json!(33)),
        ("/steps/0/fields/3/input/minimum", json!(65536)),
        ("/steps/0/fields/5/input/max_items", json!(129)),
        ("/steps/1/id", json!("connection")),
        ("/steps/1/fields/0/key", json!("tenant")),
    ] {
        let mut schema = spec_value();
        *schema.pointer_mut(pointer).unwrap() = value;
        let error = serde_json::from_value::<SetupSpec>(schema)
            .unwrap()
            .validate()
            .unwrap_err();
        assert!(
            !format!("{error:?} {error}").contains("SENTINEL"),
            "{pointer}"
        );
    }
}
fn minimal(input: Value) -> SetupSpec {
    serde_json::from_value(json!({"schema_version":1,"title":"Setup","description":"","steps":[{"id":"main","title":"Main","description":"","fields":[{"key":"value","label":"Value","help":"","required":false,"input":input}]}]})).unwrap()
}
#[test]
fn choice_values_are_safe_for_owned_prompt_rendering() {
    let schema = minimal(
        json!({"type":"choice","options":[{"value":"\u{1b}[31mSENTINEL","label":"Normal"}]}),
    );
    assert!(schema.validate().is_err());
}
#[test]
fn budget_limits_cover_spec_answers_final_defaults_and_json_depth() {
    let mut schema = minimal(json!({"type":"json"}));
    let mut nested = json!(true);
    for _ in 0..16 {
        nested = json!([nested]);
    }
    assert!(
        schema
            .questions(&BTreeMap::from([("value".into(), nested.clone())]))
            .is_err()
    );
    schema.steps[0].fields[0].default = Some(nested);
    assert!(schema.validate().is_err());
    schema.steps[0].fields[0].default = Some(json!("x".repeat(65536)));
    assert!(schema.validate().is_err());
    let schema = minimal(json!({"type":"json"}));
    assert!(
        schema
            .resolve(&BTreeMap::from([(
                "value".into(),
                json!("x".repeat(65536))
            )]))
            .is_err()
    );
    let schema = minimal(json!({"type":"string_list","min_items":1,"max_items":2}));
    for answer in [json!([]), json!(["a", "b", "c"]), json!(["x".repeat(4097)])] {
        assert!(
            schema
                .questions(&BTreeMap::from([("value".into(), answer)]))
                .is_err()
        );
    }
    let schema = minimal(json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX}));
    for answer in [json!(1.0), json!(u64::MAX)] {
        assert!(
            schema
                .resolve(&BTreeMap::from([("value".into(), answer)]))
                .is_err()
        );
    }
    assert!(
        schema
            .resolve(&BTreeMap::from([("value".into(), json!(i64::MIN))]))
            .is_ok()
    );
}
#[test]
fn exact_declared_counts_and_text_character_limits_are_enforced() {
    let mut schema = minimal(json!({"type":"text","min_length":0,"max_length":4096}));
    let field = schema.steps[0].fields[0].clone();
    schema.steps[0].fields = (0..128)
        .map(|i| {
            let mut f = field.clone();
            f.key = format!("f{i}");
            f
        })
        .collect();
    schema.validate().unwrap();
    schema.steps[0].fields.push(field);
    assert!(schema.validate().is_err());
    let mut schema = minimal(json!({"type":"credential"}));
    let field = schema.steps[0].fields[0].clone();
    schema.steps[0].fields = (0..16)
        .map(|i| {
            let mut f = field.clone();
            f.key = format!("f{i}");
            f
        })
        .collect();
    schema.validate().unwrap();
    schema.steps[0].fields.push(field);
    assert!(schema.validate().is_err());
    let schema = minimal(json!({"type":"text","min_length":1,"max_length":2}));
    assert!(
        schema
            .resolve(&BTreeMap::from([("value".into(), json!("éé"))]))
            .is_ok()
    );
    assert!(
        schema
            .resolve(&BTreeMap::from([("value".into(), json!("ééé"))]))
            .is_err()
    );
}
#[test]
fn missing_earlier_values_do_not_activate_conditions_and_null_skips_defaults() {
    let mut schema = minimal(json!({"type":"boolean"}));
    let mut dependent = schema.steps[0].fields[0].clone();
    dependent.key = "dependent".into();
    dependent.required = true;
    dependent.when = Some(permesh_provider_sdk::setup::Condition {
        field: "value".into(),
        equals: json!(true),
    });
    schema.steps[0].fields.push(dependent);
    assert_eq!(schema.questions(&BTreeMap::new()).unwrap().len(), 1);
    assert!(schema.resolve(&BTreeMap::new()).is_ok());
    assert!(
        schema
            .questions(&BTreeMap::from([("dependent".into(), Value::Null)]))
            .is_err()
    );
    schema.steps[0].fields[0].default = Some(json!(true));
    assert_eq!(schema.questions(&BTreeMap::new()).unwrap().len(), 2);
    assert!(schema.resolve(&BTreeMap::new()).is_err());
    assert!(
        schema
            .resolve(&BTreeMap::from([("value".into(), Value::Null)]))
            .unwrap()
            .configuration
            .is_empty()
    );
}

#[test]
fn combined_defaults_and_supplied_values_cannot_exceed_normalized_budget() {
    let mut schema = minimal(json!({"type":"json"}));
    let field = schema.steps[0].fields[0].clone();
    for index in 0..40 {
        let mut defaulted = field.clone();
        defaulted.key = format!("default{index}");
        defaulted.default = Some(json!("d".repeat(1000)));
        schema.steps[0].fields.push(defaulted);
    }
    schema.validate().unwrap();
    let answers = BTreeMap::from([("value".into(), json!("a".repeat(30000)))]);
    assert!(schema.questions(&answers).is_err());
    assert!(schema.resolve(&answers).is_err());
}
#[test]
fn declared_step_and_choice_count_edges_and_credential_reference_boundary() {
    let mut schema = minimal(json!({"type":"boolean"}));
    let step = schema.steps[0].clone();
    schema.steps = (0..32)
        .map(|index| {
            let mut step = step.clone();
            step.id = format!("step{index}");
            step.fields[0].key = format!("field{index}");
            step
        })
        .collect();
    schema.validate().unwrap();
    schema.steps.push(step);
    assert!(schema.validate().is_err());
    let options: Vec<_> = (0..128)
        .map(|i| json!({"value":i.to_string(),"label":format!("Option {i}")}))
        .collect();
    let mut schema = minimal(json!({"type":"choice","options":options}));
    schema.validate().unwrap();
    if let Input::Choice { options } = &mut schema.steps[0].fields[0].input {
        options.push(permesh_provider_sdk::setup::Choice {
            value: "extra".into(),
            label: "Extra".into(),
        });
    }
    assert!(schema.validate().is_err());
    let schema = minimal(json!({"type":"credential"}));
    // This pure evaluator bounds strings; the CLI owns reference syntax checks.
    let resolved = schema
        .resolve(&BTreeMap::from([(
            "value".into(),
            json!("SENTINEL-not-a-reference"),
        )]))
        .unwrap();
    assert_eq!(resolved.credentials["value"], "SENTINEL-not-a-reference");
    assert!(!format!("{resolved:?}").contains("SENTINEL"));
    assert!(
        schema
            .resolve(&BTreeMap::from([("value".into(), json!(""))]))
            .is_err()
    );
    assert!(
        schema
            .resolve(&BTreeMap::from([("value".into(), Value::Null)]))
            .unwrap()
            .credentials
            .is_empty()
    );
}
