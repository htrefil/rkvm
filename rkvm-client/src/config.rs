use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt::{self, Formatter};
use std::path::PathBuf;
use tokio_rustls::rustls::ServerName;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub server: Server,
    pub certificate: PathBuf,
}
pub struct Server {
    pub hostname: ServerName,
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

        let hostname = hostname.try_into().map_err(E::custom)?;
        let port = port.parse().map_err(E::custom)?;

        Ok(Server { hostname, port })
    }
}
