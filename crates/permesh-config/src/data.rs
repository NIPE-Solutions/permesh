// SPDX-License-Identifier: MIT
use crate::{Error, Result};
use serde::{Deserialize, de::DeserializeOwned};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};
const MAX_ANSWERS_BYTES: usize = 65_536;

pub(crate) fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    if !std::fs::metadata(path).map_err(|_| Error::Read)?.is_file() {
        return Err(Error::Read);
    }
    let file = File::open(path).map_err(|_| Error::Read)?;
    if !file.metadata().map_err(|_| Error::Read)?.is_file() {
        return Err(Error::Read);
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Read)?;
    if bytes.len() > limit {
        return Err(Error::TooLarge);
    }
    Ok(bytes)
}
pub(crate) fn parse<T: DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T> {
    let options = serde_saphyr::options! {
        with_snippet: false,
        reject_unsupported_tags: true,
        merge_keys: serde_saphyr::MergeKeyPolicy::Error,
        budget: serde_saphyr::budget! { max_documents: 1, max_depth: 16, flow_nesting_limit: 16, max_events: 100_000, max_nodes: 30_000, max_total_scalar_bytes: limit, max_aliases: 0, max_anchors: 0, max_inclusion_depth: 0 },
    };
    serde_saphyr::from_slice_with_options(bytes, options).map_err(|error| match error.location() {
        Some(location) => Error::ParseAt {
            line: location.line(),
            column: location.column(),
        },
        None => Error::Parse,
    })
}
/// Load local setup answers as bounded, strict YAML or JSON. Values never execute code.
pub fn load_setup_answers(path: &Path) -> Result<BTreeMap<String, serde_json::Value>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Answers {
        version: u32,
        answers: BTreeMap<String, serde_json::Value>,
    }
    let bytes = read_file(path, MAX_ANSWERS_BYTES).map_err(|error| {
        if matches!(error, Error::TooLarge) {
            Error::Validation("setup answers exceed the 64 KiB limit")
        } else {
            error
        }
    })?;
    let file: Answers = parse(&bytes, MAX_ANSWERS_BYTES)?;
    if file.version != 1 {
        return Err(Error::Validation("setup answer version must be 1"));
    }
    Ok(file.answers)
}

/// Parse one structured setup answer with the same duplicate-key and nesting rules as files.
pub fn parse_setup_value(input: &str) -> Result<serde_json::Value> {
    if input.len() > MAX_ANSWERS_BYTES {
        return Err(Error::Validation("setup answer exceeds 64 KiB"));
    }
    parse(input.as_bytes(), MAX_ANSWERS_BYTES)
}
