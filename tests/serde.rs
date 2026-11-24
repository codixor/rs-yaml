#![cfg(feature = "serde")]

use rs_yaml::{from_str, to_string};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeMap};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct Config {
    enabled: bool,
    names: Vec<String>,
    mode: Mode,
    retries: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
enum Mode {
    Fast,
    Timed(u32),
}

#[test]
fn typed_values_round_trip() {
    let config: Config =
        from_str("enabled: true\nnames: [first, 'true']\nmode: Fast\nretries: null").unwrap();
    assert_eq!(config.names, ["first", "true"]);
    for mode in [Mode::Fast, Mode::Timed(30)] {
        let config = Config {
            mode,
            ..config.clone()
        };
        assert_eq!(
            from_str::<Config>(&to_string(&config).unwrap()).unwrap(),
            config
        );
    }
}

#[test]
fn typed_loading_checks_the_whole_input_and_redacts_custom_errors() {
    for yaml in ["enabled: true\nignored: {k: 1, k: 2}", "null\n---\n"] {
        assert!(from_str::<serde::de::IgnoredAny>(yaml).is_err());
    }
    let error = from_str::<u32>("'private-fixture-marker'").unwrap_err();
    assert!(!format!("{error} {error:?}").contains("private-fixture-marker"));

    #[derive(Debug)]
    struct Refuse;
    impl<'de> Deserialize<'de> for Refuse {
        fn deserialize<D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
            Err(serde::de::Error::custom("private-fixture-marker"))
        }
    }
    let error = from_str::<Refuse>("x").unwrap_err();
    assert!(!format!("{error} {error:?}").contains("private-fixture-marker"));
}

#[test]
fn custom_serializers_cannot_emit_duplicate_keys_or_leak_their_errors() {
    struct Duplicate;
    impl Serialize for Duplicate {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("key", &1)?;
            map.serialize_entry("key", &2)?;
            map.end()
        }
    }
    assert!(to_string(&Duplicate).is_err());

    struct Refuse;
    impl Serialize for Refuse {
        fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("private-fixture-marker"))
        }
    }
    let error = to_string(&Refuse).unwrap_err();
    assert!(!format!("{error} {error:?}").contains("private-fixture-marker"));
}

#[test]
fn unix_file_mode_spelling_for_u32() {
    #[derive(Deserialize)]
    struct Config {
        mode: u32,
    }

    assert!(from_str::<Config>("mode: 0600").is_err());
    for yaml in ["mode: 0o600", "mode: 384"] {
        assert_eq!(from_str::<Config>(yaml).unwrap().mode, 0o600);
    }
}
