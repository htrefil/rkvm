use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt::{self, Formatter};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use tokio_rustls::rustls::ServerName;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub server: Server,
    pub certificate: PathBuf,
    pub password: String,
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
        // Parsing IPv6 socket addresses can get quite hairy, so let the SocketAddr parser do it for us.
        if let Ok(socket_addr) = SocketAddr::from_str(data) {
            return Ok(Server {
                hostname: ServerName::IpAddress(socket_addr.ip()),
                port: socket_addr.port(),
            });
        }

        let (hostname, port) = data
            .split_once(':')
            .ok_or_else(|| E::custom("No port provided"))?;

        let hostname = hostname.try_into().map_err(E::custom)?;
        let port = port.parse().map_err(E::custom)?;

        Ok(Server { hostname, port })
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv6Addr;

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
            hostname: "127.0.0.1".try_into().unwrap(),
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
            hostname: "::1".try_into().unwrap(),
            port: 8523,
        };

        assert_eq!(parsed.hostname, expected.hostname);
        assert_eq!(parsed.port, expected.port);

        let parsed_ip = match parsed.hostname {
            ServerName::IpAddress(parsed_ip) => parsed_ip,
            _ => unreachable!(),
        };

        assert_eq!(parsed_ip, Ipv6Addr::from_str("::1").unwrap());
    }
}
