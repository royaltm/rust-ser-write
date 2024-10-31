#![cfg(any(feature = "std", feature = "alloc"))]
use serde::{Serialize, Deserialize};
use ser_write_json::*;
use serde_json::{json, Value};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Test<'a> {
    name: &'a str,
    age: u8,
    score: i32,
    height: f32,
    phones: Vec<&'a str>,
    human: bool,
}

#[test]
fn deserialize_any() {
    let john_val = json!({
        "name": "John Doe",
        "age": 43,
        "score": -999,
        "height": 5.75,
        "phones": [
            "+44 1234567",
            "+44 2345678"
        ],
        "human": true
    });
    let john_test = Test {
        name: "John Doe",
        age: 43,
        score: -999,
        height: 5.75,
        phones: vec!["+44 1234567", "+44 2345678"],
        human: true
    };
    let s = to_string(&john_val).unwrap();
    assert_eq!(s,
        r#"{"age":43,"height":5.75,"human":true,"name":"John Doe","phones":["+44 1234567","+44 2345678"],"score":-999}"#
    );
    let mut vec = s.clone().into_bytes();
    let test: Test = from_mut_slice(&mut vec).unwrap();
    assert_eq!(test, john_test);
    let mut vec = s.clone().into_bytes();
    let value: Value = from_mut_slice(&mut vec).unwrap();
    assert_eq!(value, john_val);

    let s = to_string(&john_test).unwrap();
    assert_eq!(s,
        r#"{"name":"John Doe","age":43,"score":-999,"height":5.75,"phones":["+44 1234567","+44 2345678"],"human":true}"#
    );
    let mut vec = s.clone().into_bytes();
    let test: Test = from_mut_slice(&mut vec).unwrap();
    assert_eq!(test, john_test);
    let mut vec = s.clone().into_bytes();
    let value: Value = from_mut_slice(&mut vec).unwrap();
    assert_eq!(value, john_val);
}
