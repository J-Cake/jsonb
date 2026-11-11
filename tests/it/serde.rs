// Copyright 2023 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt;

use jsonb::from_raw_jsonb;
use jsonb::to_owned_jsonb;
use jsonb::OwnedJsonb;
use jsonb::Value;
use serde::de::Visitor;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;

#[derive(Debug, PartialEq)]
struct Bytes(Vec<u8>);

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BytesVisitor;

        impl Visitor<'_> for BytesVisitor {
            type Value = Bytes;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bytes")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Bytes, E> {
                Ok(Bytes(v.to_vec()))
            }

            fn visit_str<E>(self, v: &str) -> Result<Bytes, E> {
                Ok(Bytes(v.as_bytes().to_vec()))
            }
        }
        deserializer.deserialize_bytes(BytesVisitor)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct WithBytes {
    id: u32,
    data: Bytes,
    list: Vec<Bytes>,
}

#[test]
fn test_serde_bytes() {
    // Bytes are encoded as a binary extension value.
    let bytes = Bytes(vec![0, 1, 2, 255]);
    let owned = to_owned_jsonb(&bytes).unwrap();
    assert_eq!(
        owned.clone().to_vec(),
        Value::Binary(&[0, 1, 2, 255]).to_vec()
    );
    assert_eq!(from_raw_jsonb::<Bytes>(&owned.as_raw()).unwrap(), bytes);

    let empty = Bytes(vec![]);
    let owned = to_owned_jsonb(&empty).unwrap();
    assert_eq!(from_raw_jsonb::<Bytes>(&owned.as_raw()).unwrap(), empty);

    // Nested in objects and arrays.
    let value = WithBytes {
        id: 7,
        data: Bytes(vec![1, 2, 3]),
        list: vec![Bytes(vec![]), Bytes(vec![9, 8]), Bytes(b"abc".to_vec())],
    };
    let owned = to_owned_jsonb(&value).unwrap();
    assert_eq!(from_raw_jsonb::<WithBytes>(&owned.as_raw()).unwrap(), value);

    // Strings can still be read as bytes.
    let owned = r#""abc""#.parse::<OwnedJsonb>().unwrap();
    assert_eq!(
        from_raw_jsonb::<Bytes>(&owned.as_raw()).unwrap(),
        Bytes(b"abc".to_vec())
    );
}

#[derive(Debug, PartialEq, Deserialize)]
struct Inner {
    x: i32,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Partial {
    a: i32,
    inner: Inner,
    items: Vec<Inner>,
    z: String,
}

#[test]
fn test_serde_ignore_unknown_fields() {
    // Unknown fields of every type are skipped, on all nesting levels.
    let owned = r#"{
        "a": 1,
        "skip_null": null,
        "skip_bool": true,
        "skip_num": 12.5,
        "skip_str": "ignored",
        "skip_arr": [1, "two", [3, {"four": 4}], null],
        "skip_obj": {"k": {"n": [1, 2, 3]}, "s": "v"},
        "inner": {"skip": {"deep": [true, false]}, "x": 2, "also": "skip"},
        "items": [{"x": 3, "u": [1, 2]}, {"u": {"v": "w"}, "x": 4}],
        "z": "end"
    }"#
    .parse::<OwnedJsonb>()
    .unwrap();

    let expected = Partial {
        a: 1,
        inner: Inner { x: 2 },
        items: vec![Inner { x: 3 }, Inner { x: 4 }],
        z: "end".to_string(),
    };
    assert_eq!(
        from_raw_jsonb::<Partial>(&owned.as_raw()).unwrap(),
        expected
    );

    // Whole values can be ignored as well.
    for s in ["null", "1", r#""a""#, "[1,[2,3]]", r#"{"a":{"b":[1]}}"#] {
        let owned = s.parse::<OwnedJsonb>().unwrap();
        from_raw_jsonb::<serde::de::IgnoredAny>(&owned.as_raw()).unwrap();
    }
}
