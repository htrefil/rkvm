use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt::{self, Formatter};
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub server: Server,
    pub certificate: PathBuf,
    pub password: String,
}

pub struct Server {
    pub hostname: String,
    pub port: u16,
}

impl<'de> Deserialize<'de> for Server {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ServerVisitor)
    }
}

struct ServerVisitor;

impl<'de> Visitor<'de> for ServerVisitor {
    type Value = Server;

    fn expecting(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "a server description (hostname:port)")
    }

    fn visit_str<E>(self, data: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let (hostname, port) = data
            .rsplit_once(':')
            .ok_or_else(|| E::custom("No port provided"))?;

        let hostname = hostname.to_owned();
        let port = port.parse().map_err(E::custom)?;

        Ok(Server { hostname, port })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Data {
        server: Server,
    }

    #[test]
    fn server_dns() {
        let parsed = toml::from_str::<Data>(r#"server = "example.com:8523""#)
            .unwrap()
            .server;
        let expected = Server {
            hostname: "example.com".try_into().unwrap(),
            port: 8523,
        };

        assert_eq!(parsed.hostname, expected.hostname);
        assert_eq!(parsed.port, expected.port);
    }

    #[test]
    fn server_ipv4() {
        let parsed = toml::from_str::<Data>(r#"server = "127.0.0.1:8523""#)
            .unwrap()
            .server;
        let expected = Server {
            hostname: "127.0.0.1".to_owned(),
            port: 8523,
        };

        assert_eq!(parsed.hostname, expected.hostname);
        assert_eq!(parsed.port, expected.port);
    }

    #[test]
    fn server_ipv6() {
        let parsed = toml::from_str::<Data>(r#"server = "[::1]:8523""#)
            .unwrap()
            .server;
        let expected = Server {
            hostname: "[::1]".to_owned(),
            port: 8523,
        };

        assert_eq!(parsed.hostname, expected.hostname);
        assert_eq!(parsed.port, expected.port);
    }

    #[test]
    fn example_parses() {
        let config = include_str!("../../example/client.toml");
        toml::from_str::<Config>(config).unwrap();
    }
}
