use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt::{self, Formatter};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use tokio_rustls::rustls::pki_types::ServerName;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub server: Server,
    // TLS is disabled if not set.
    pub certificate: Option<PathBuf>,
    pub password: String,
    #[serde(default)]
    pub output: Output,
}

#[derive(Deserialize, Default)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum Output {
    #[default]
    Uinput,
    Usb(Usb),
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Usb {
    pub udc: Option<String>,
    #[serde(default = "default_vendor_id")]
    pub vendor_id: u16,
    #[serde(default = "default_product_id")]
    pub product_id: u16,
    #[serde(default = "default_manufacturer")]
    pub manufacturer: String,
    #[serde(default = "default_product")]
    pub product: String,
    pub serial_number: Option<String>,
}

// Linux Foundation.
fn default_vendor_id() -> u16 {
    0x1d6b
}

// Multifunction Composite Gadget.
fn default_product_id() -> u16 {
    0x0104
}

fn default_manufacturer() -> String {
    "rkvm".to_owned()
}

fn default_product() -> String {
    "rkvm virtual input".to_owned()
}

pub struct Server {
    pub hostname: ServerName<'static>,
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
                hostname: ServerName::IpAddress(socket_addr.ip().into()),
                port: socket_addr.port(),
            });
        }

        let (hostname, port) = data
            .split_once(':')
            .ok_or_else(|| E::custom("No port provided"))?;

        let hostname = hostname.to_owned().try_into().map_err(E::custom)?;
        let port = port.parse().map_err(E::custom)?;

        Ok(Server { hostname, port })
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv6Addr};

    use super::*;

    #[derive(Deserialize)]
    struct Data {
        server: Server,
    }

    #[derive(Deserialize)]
    struct OutputData {
        #[serde(default)]
        output: Output,
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
            ServerName::IpAddress(parsed_ip) => IpAddr::from(parsed_ip),
            _ => unreachable!(),
        };

        assert_eq!(parsed_ip, Ipv6Addr::from_str("::1").unwrap());
    }

    #[test]
    fn output_default() {
        let parsed = toml::from_str::<OutputData>("").unwrap();
        assert!(matches!(parsed.output, Output::Uinput));
    }

    #[test]
    fn output_usb() {
        let parsed = toml::from_str::<OutputData>(
            r#"
            [output]
            mode = "usb"
            udc = "fe980000.usb"
            "#,
        )
        .unwrap();

        let usb = match parsed.output {
            Output::Usb(usb) => usb,
            _ => unreachable!(),
        };

        assert_eq!(usb.udc.as_deref(), Some("fe980000.usb"));
        assert_eq!(usb.vendor_id, default_vendor_id());
        assert_eq!(usb.product_id, default_product_id());
        assert_eq!(usb.manufacturer, default_manufacturer());
        assert_eq!(usb.product, default_product());
        assert_eq!(usb.serial_number, None);
    }

    #[test]
    fn output_usb_identity() {
        let parsed = toml::from_str::<OutputData>(
            r#"
            [output]
            mode = "usb"
            vendor-id = 0x046d
            product-id = 0xc31c
            manufacturer = "Logitech"
            product = "USB Keyboard"
            serial-number = "1234"
            "#,
        )
        .unwrap();

        let usb = match parsed.output {
            Output::Usb(usb) => usb,
            _ => unreachable!(),
        };

        assert_eq!(usb.vendor_id, 0x046d);
        assert_eq!(usb.product_id, 0xc31c);
        assert_eq!(usb.manufacturer, "Logitech");
        assert_eq!(usb.product, "USB Keyboard");
        assert_eq!(usb.serial_number.as_deref(), Some("1234"));
    }

    #[test]
    fn certificate_optional() {
        let config = toml::from_str::<Config>(
            r#"
            server = "example.com:5258"
            password = "123456789"
            "#,
        )
        .unwrap();

        assert_eq!(config.certificate, None);
    }

    #[test]
    fn example_parses() {
        let config = include_str!("../../example/client.toml");
        toml::from_str::<Config>(config).unwrap();
    }
}
