use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt::{self, Formatter};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub server: Server,
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
        let err = || E::custom("Invalid server description");

        let mut split = data.split(':');
        let hostname = split.next().ok_or_else(err)?;
        let port = split
            .next()
            .and_then(|data| data.parse().ok())
            .ok_or_else(err)?;

        if split.next().is_some() {
            return Err(E::custom("Extraneous data"));
        }

        Ok(Server {
            hostname: hostname.to_owned(),
            port,
        })
    }
}
