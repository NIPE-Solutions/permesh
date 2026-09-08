// SPDX-License-Identifier: MIT
use crate::{MAX_FRAME_BYTES, MAX_TRANSCRIPT_BYTES, ProtocolError};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value};
use std::{
    fmt,
    io::{BufRead, Read},
};

pub(crate) fn read_json_frame(
    reader: &mut impl BufRead,
    total: &mut usize,
) -> Result<Option<serde_json::Value>, crate::ProtocolError> {
    let remaining = MAX_TRANSCRIPT_BYTES.saturating_sub(*total);
    let limit = MAX_FRAME_BYTES.min(remaining);
    let mut frame = Vec::new();
    // Take bounds read_until's allocation and consumption even without a delimiter.
    // One extra byte distinguishes an exact limit followed by EOF from overflow.
    reader
        .take((limit + 1) as u64)
        .read_until(b'\n', &mut frame)
        .map_err(|_| ProtocolError::Io)?;
    *total = total.saturating_add(frame.len());
    if *total > MAX_TRANSCRIPT_BYTES {
        return Err(ProtocolError::TranscriptLimit);
    }
    if frame.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::FrameLimit);
    }
    if frame.is_empty() {
        return Ok(None);
    }
    if frame.last() != Some(&b'\n') {
        return Err(ProtocolError::Truncated);
    }
    serde_json::from_slice::<UniqueValue>(&frame)
        .map(|value| Some(value.0))
        .map_err(|_| ProtocolError::Json)
}

// Deserialize recursively before constructing Value: Value itself silently keeps
// the last occurrence of a duplicate key. serde_json retains its recursion limit.
struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueVisitor)
    }
}

struct UniqueVisitor;

impl<'de> Visitor<'de> for UniqueVisitor {
    type Value = UniqueValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Bool(value)))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        serde_json::Number::from_f64(value)
            .map(|number| UniqueValue(Value::Number(number)))
            .ok_or_else(|| E::custom("invalid number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value.to_owned())))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value)))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<UniqueValue>()? {
            values.push(value.0);
        }
        Ok(UniqueValue(Value::Array(values)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Self::Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate object key"));
            }
            values.insert(key, object.next_value::<UniqueValue>()?.0);
        }
        Ok(UniqueValue(Value::Object(values)))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{MAX_FRAME_BYTES, MAX_TRANSCRIPT_BYTES, ProtocolError};
    use std::io::{self, BufReader, Cursor, Read};

    fn decode(bytes: &[u8]) -> Result<Option<serde_json::Value>, ProtocolError> {
        read_json_frame(&mut Cursor::new(bytes), &mut 0)
    }

    #[test]
    fn frames_stop_at_lf_and_count_all_bytes() {
        let mut input = Cursor::new(b"{\"a\":1}\n[true,null,-2,1.5,\"x\"]\n");
        let mut total = 0;
        assert_eq!(
            read_json_frame(&mut input, &mut total).unwrap(),
            Some(serde_json::json!({"a":1}))
        );
        assert_eq!(total, 8);
        assert_eq!(
            read_json_frame(&mut input, &mut total).unwrap(),
            Some(serde_json::json!([true, null, -2, 1.5, "x"]))
        );
        assert_eq!(total, input.get_ref().len());
        assert_eq!(read_json_frame(&mut input, &mut total).unwrap(), None);
    }

    #[test]
    fn frame_limit_includes_lf() {
        let mut bytes = vec![b' '; MAX_FRAME_BYTES - 3];
        bytes.extend_from_slice(b"{}\n");
        assert!(decode(&bytes).unwrap().is_some());
        bytes.insert(0, b' ');
        assert_eq!(decode(&bytes), Err(ProtocolError::FrameLimit));
    }

    #[test]
    fn transcript_limit_counts_across_frames_and_allows_eof_at_limit() {
        let mut total = MAX_TRANSCRIPT_BYTES - 3;
        assert!(
            read_json_frame(&mut Cursor::new(b"{}\n"), &mut total)
                .unwrap()
                .is_some()
        );
        assert_eq!(total, MAX_TRANSCRIPT_BYTES);
        assert_eq!(read_json_frame(&mut Cursor::new(b""), &mut total), Ok(None));
        assert_eq!(
            read_json_frame(&mut Cursor::new(b"{}\n"), &mut total),
            Err(ProtocolError::TranscriptLimit)
        );
        let mut total = MAX_TRANSCRIPT_BYTES - 2;
        assert_eq!(
            read_json_frame(&mut Cursor::new(b"{}\n"), &mut total),
            Err(ProtocolError::TranscriptLimit)
        );
    }

    struct Endless {
        read: usize,
    }
    impl Read for Endless {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            out.fill(b'x');
            self.read += out.len();
            Ok(out.len())
        }
    }

    #[test]
    fn endless_input_is_bounded_by_frame_and_remaining_transcript_budget() {
        let mut input = BufReader::with_capacity(1, Endless { read: 0 });
        assert_eq!(
            read_json_frame(&mut input, &mut 0),
            Err(ProtocolError::FrameLimit)
        );
        assert_eq!(input.get_ref().read, MAX_FRAME_BYTES + 1);
        let mut input = BufReader::with_capacity(1, Endless { read: 0 });
        assert_eq!(
            read_json_frame(&mut input, &mut (MAX_TRANSCRIPT_BYTES - 7)),
            Err(ProtocolError::TranscriptLimit)
        );
        assert_eq!(input.get_ref().read, 8);
    }

    #[test]
    fn rejects_duplicate_keys_at_any_depth_after_unescaping() {
        for input in [
            "{\"a\":1,\"a\":2}\n",
            "{\"outer\":[{\"a\":1,\"a\":2}]}\n",
            "{\"a\":1,\"\\u0061\":2}\n",
            "{\"outer\":{\"a\":1,\"\\u0061\":2}}\n",
        ] {
            assert_eq!(decode(input.as_bytes()), Err(ProtocolError::Json));
        }
        assert!(decode(b"[{\"a\":1},{\"a\":2}]\n").unwrap().is_some());
    }

    #[test]
    fn rejects_invalid_utf8_malformed_json_and_excessive_nesting() {
        for input in [
            b"\"\xff\"\n".as_slice(),
            b"{\n",
            b"{} {}\n",
            b"\n",
            b"1e999\n",
        ] {
            assert_eq!(decode(input), Err(ProtocolError::Json));
        }
        let deep = format!("{}0{}\n", "[".repeat(200), "]".repeat(200));
        assert_eq!(decode(deep.as_bytes()), Err(ProtocolError::Json));
    }

    #[test]
    fn requires_lf_even_for_otherwise_valid_json() {
        assert_eq!(decode(b"{}"), Err(ProtocolError::Truncated));
        assert_eq!(decode(b" "), Err(ProtocolError::Truncated));
        assert_eq!(decode(b""), Ok(None));
    }

    struct Failing;
    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("SECRET_TOKEN"))
        }
    }

    #[test]
    fn errors_never_include_input_or_raw_io_messages() {
        let error = read_json_frame(&mut BufReader::new(Failing), &mut 0).unwrap_err();
        assert_eq!(error, ProtocolError::Io);
        for error in [error, decode(b"SECRET_TOKEN\n").unwrap_err()] {
            assert!(!error.to_string().contains("SECRET_TOKEN"));
            assert!(!format!("{error:?}").contains("SECRET_TOKEN"));
        }
    }
}
