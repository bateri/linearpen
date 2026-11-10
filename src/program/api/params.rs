//! The params of a call, read as the Rust type that names what the call takes. A call is given only
//! the members it names (`deny_unknown_fields`): a page that sends one the program does not know is
//! wrong, and is told so before any work runs.

use serde::de::DeserializeOwned;
use serde_json::Value;

/// The params of a call read as `P`, or the words that say what does not fit.
pub fn parse<P: DeserializeOwned>(params: Value) -> Result<P, String> {
    // A call that takes nothing may be sent with no params at all.
    let params = if params.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        params
    };
    serde_json::from_value(params).map_err(|why| why.to_string())
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Limit {
        first: Option<f64>,
    }

    #[test]
    fn params_that_fit_are_read() {
        assert_eq!(
            parse::<Limit>(json!({ "first": 2 })),
            Ok(Limit { first: Some(2.0) })
        );
        assert_eq!(parse::<Limit>(Value::Null), Ok(Limit { first: None }));
    }

    #[test]
    fn a_member_the_call_does_not_take_is_refused_by_name() {
        let why = parse::<Limit>(json!({ "first": 2, "speed": 1 })).unwrap_err();
        assert!(why.contains("speed"), "{why}");
        assert!(parse::<Limit>(json!({ "first": "fast" })).is_err());
    }
}
