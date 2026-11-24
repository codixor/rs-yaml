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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct Strict {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    level: Option<Level>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
enum Level {
    Low,
    High,
}

#[test]
fn booleans_are_true_or_false_only() {
    for yaml in ["enabled: true", "enabled: false"] {
        assert!(from_str::<Strict>(yaml).unwrap().enabled.is_some());
    }
    for yaml in [
        "enabled: yes",
        "enabled: on",
        "enabled: no",
        "enabled: off",
        "enabled: y",
    ] {
        let error = from_str::<Strict>(yaml).unwrap_err().to_string();
        assert!(error.contains("invalid boolean"), "{yaml}: {error}");
    }
}

#[test]
fn tags_outside_the_core_schema_are_refused() {
    for yaml in [
        "name: !vault private-fixture-marker",
        "name: !<tag:x.io,2026:t> v",
        "level: !Low",
    ] {
        let error = from_str::<Strict>(yaml).unwrap_err().to_string();
        assert!(error.contains("unsupported YAML tag"), "{yaml}: {error}");
        assert!(
            !error.contains("vault") && !error.contains("x.io"),
            "{error}"
        );
    }
    assert_eq!(
        from_str::<Strict>("name: !!str 123")
            .unwrap()
            .name
            .as_deref(),
        Some("123")
    );
}

#[test]
fn serde_errors_name_the_setting_but_never_the_value() {
    let cases = [
        (
            "tokne: private-fixture-marker",
            "unknown field `tokne`, expected one of",
        ),
        ("port: private-fixture-marker", "invalid value for u16"),
        ("port: 70000", "u16"),
        (
            "level: private-fixture-marker",
            "unknown variant, expected one of `Low`, `High`",
        ),
    ];
    for (yaml, wanted) in cases {
        let error = from_str::<Strict>(yaml).unwrap_err();
        let text = format!("{error} {error:?}");
        assert!(text.contains(wanted), "{yaml}: {text}");
        assert!(
            !text.contains("private-fixture-marker") && !text.contains("70000"),
            "{text}"
        );
    }
}
