// SPDX-License-Identifier: MIT
use super::*;
use std::collections::BTreeMap;
pub(super) fn parse(bytes: &[u8], host: &str, state: &str) -> Result<Secret, FlowError> {
    if bytes.len() > 16 * 1024 || !bytes.is_ascii() {
        return Err(FlowError::Callback);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| FlowError::Callback)?;
    let head = text.strip_suffix("\r\n\r\n").ok_or(FlowError::Callback)?;
    let mut lines = head.split("\r\n");
    let request = lines.next().ok_or(FlowError::Callback)?;
    let mut parts = request.split(' ');
    if parts.next() != Some("GET") {
        return Err(FlowError::Callback);
    }
    let target = parts.next().ok_or(FlowError::Callback)?;
    if parts.next() != Some("HTTP/1.1") || parts.next().is_some() {
        return Err(FlowError::Callback);
    }
    let mut headers = BTreeMap::new();
    for line in lines {
        let (key, value) = line.split_once(':').ok_or(FlowError::Callback)?;
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.bytes().any(|b| b.is_ascii_control() && b != b'\t')
            || headers.len() >= 32
            || headers
                .insert(key.to_ascii_lowercase(), value.trim())
                .is_some()
        {
            return Err(FlowError::Callback);
        }
    }
    if headers.get("host").copied() != Some(host)
        || headers.contains_key("transfer-encoding")
        || headers.get("content-length").is_some_and(|v| *v != "0")
    {
        return Err(FlowError::Callback);
    }
    let query = target
        .strip_prefix("/oauth/callback?")
        .ok_or(FlowError::Callback)?;
    if query.len() > 12 * 1024 || query.contains('#') {
        return Err(FlowError::Callback);
    }
    for (index, byte) in query.bytes().enumerate() {
        if byte == b'%'
            && (index + 2 >= query.len()
                || !query.as_bytes()[index + 1..index + 3]
                    .iter()
                    .all(u8::is_ascii_hexdigit))
        {
            return Err(FlowError::Callback);
        }
    }
    let mut values = BTreeMap::new();
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        if !matches!(
            key.as_ref(),
            "code"
                | "state"
                | "error"
                | "error_description"
                | "error_uri"
                | "scope"
                | "authuser"
                | "prompt"
                | "hd"
        ) || value.len() > 4096
            || !value.bytes().all(|b| b.is_ascii() && !b.is_ascii_control())
            || values
                .insert(key.into_owned(), Zeroizing::new(value.into_owned()))
                .is_some()
        {
            return Err(FlowError::Callback);
        }
    }
    let observed = values.get("state").ok_or(FlowError::Callback)?;
    if observed.len() != state.len()
        || observed
            .bytes()
            .zip(state.bytes())
            .fold(0, |different, (a, b)| different | (a ^ b))
            != 0
    {
        return Err(FlowError::Callback);
    }
    if values.contains_key("error") {
        return Err(if values.contains_key("code") {
            FlowError::Callback
        } else {
            FlowError::Denied
        });
    }
    if values.contains_key("error_description") || values.contains_key("error_uri") {
        return Err(FlowError::Callback);
    }
    let mut code = values.remove("code").ok_or(FlowError::Callback)?;
    if code.is_empty() || !code.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(FlowError::Callback);
    }
    Ok(Secret::new(std::mem::take(&mut *code)))
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    fn request(query: &str) -> Vec<u8> {
        format!("GET /oauth/callback?{query} HTTP/1.1\r\nHost: 127.0.0.1:54321\r\n\r\n")
            .into_bytes()
    }
    #[test]
    fn valid_callback_binds_host_path_and_single_state_and_code() {
        let code = parse(
            &request("state=expected&code=code%2Fvalue&scope=read"),
            "127.0.0.1:54321",
            "expected",
        )
        .unwrap();
        assert_eq!(code.expose(), "code/value");
        for query in [
            "state=wrong&code=SECRET",
            "state=expected&state=expected&code=SECRET",
            "state=expected&code=SECRET&code=other",
            "state=expected&code=SECRET&redirect_uri=https%3A%2F%2Fevil.test",
            "state=expected&code=SECRET&error=denied",
            "state=expected&code=%00SECRET",
            "state=expected&code=%GG",
            "state=expected&error=denied&error_description=SECRET",
        ] {
            let err = parse(&request(query), "127.0.0.1:54321", "expected").unwrap_err();
            assert!(!format!("{err:?}").contains("SECRET"));
        }
        for altered in [
            String::from_utf8(request("state=expected&code=SECRET"))
                .unwrap()
                .replace("127.0.0.1:54321", "evil.test"),
            String::from_utf8(request("state=expected&code=SECRET"))
                .unwrap()
                .replace("/oauth/callback", "/wrong"),
            String::from_utf8(request("state=expected&code=SECRET"))
                .unwrap()
                .replace("GET ", "POST "),
        ] {
            assert!(parse(altered.as_bytes(), "127.0.0.1:54321", "expected").is_err());
        }
    }
}
