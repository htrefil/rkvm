use serde::de::{Deserializer, Error, Visitor};
use serde::Deserialize;
use std::fmt::{self, Formatter};
use std::str::FromStr;
use std::time::Duration;

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Timeout {
    #[serde(
        rename = "read-timeout",
        default = "default_timeout",
        deserialize_with = "deserialize_duration"
    )]
    pub read: Duration,
    #[serde(
        rename = "write-timeout",
        default = "default_timeout",
        deserialize_with = "deserialize_duration"
    )]
    pub write: Duration,
    #[serde(
        rename = "tls-timeout",
        default = "default_timeout",
        deserialize_with = "deserialize_duration"
    )]
    pub tls: Duration,
}

fn default_timeout() -> Duration {
    Duration::from_millis(500)
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    struct DurationVisitor;

    impl Visitor<'_> for DurationVisitor {
        type Value = Duration;

        fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
            write!(formatter, "a duration of time (for example \"500ms\")")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            humantime::Duration::from_str(v)
                .map_err(E::custom)
                .map(Into::into)
        }
    }

    deserializer.deserialize_str(DurationVisitor)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn timeout_deserialize() {
        let parsed = toml::from_str::<Timeout>(
            r#"
            read-timeout = "1s"
            write-timeout = "200ms"
            tls-timeout = "500ms"
            "#,
        )
        .unwrap();

        assert_eq!(
            parsed,
            Timeout {
                read: Duration::from_secs(1),
                write: Duration::from_millis(200),
                tls: Duration::from_millis(500),
            }
        );
    }

    #[test]
    fn timeout_missing_values() {
        let parsed = toml::from_str::<Timeout>("").unwrap();

        assert_eq!(
            parsed,
            Timeout {
                read: default_timeout(),
                write: default_timeout(),
                tls: default_timeout(),
            }
        );
    }
}
