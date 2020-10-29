use anyhow::{Context, Error};
use std::env;
use std::fmt::Write as _;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use structopt::StructOpt;
use tempfile::NamedTempFile;

fn run(
    identity_path: &Path,
    certificate_path: &Path,
    key_path: &Path,
    dns_names: &[String],
    ip_addresses: &[IpAddr],
) -> Result<(), Error> {
    if dns_names.is_empty() && ip_addresses.is_empty() {
        return Err(anyhow::anyhow!(
            "No DNS names nor IP addresses were provided"
        ));
    }

    let mut config = "[req]
prompt = no
default_bits = 2048
distinguished_name = req_distinguished_name
req_extensions = req_ext
x509_extensions = v3_req
[req_distinguished_name]
commonName = rkvm
countryName = CZ
localityName = rkvm
organizationName = rkvm
organizationalUnitName = IT
stateOrProvinceName = rkvm
emailAddress = nowhere@example.com
[req_ext]
subjectAltName = @alt_names
[v3_req]
subjectAltName = @alt_names
[alt_names]"
        .to_owned();
    for (i, name) in dns_names.iter().enumerate() {
        write!(config, "\nDNS.{} = {}", i + 1, name)?;
    }

    for (i, address) in ip_addresses.iter().enumerate() {
        write!(config, "\nIP.{} = {}", i + 1, address)?;
    }

    let mut file = NamedTempFile::new().context("Failed to open config file")?;
    file.write_all(config.as_bytes())
        .context("Failed to write to config file")?;

    let openssl = env::var_os("OPENSSL").unwrap_or_else(|| "openssl".to_owned().into());
    let code = Command::new(&openssl)
        .arg("req")
        .arg("-sha256")
        .arg("-x509")
        .arg("-nodes")
        .arg("-days")
        .arg("365")
        .arg("-newkey")
        .arg("rsa:2048")
        .arg("-keyout")
        .arg(key_path)
        .arg("-out")
        .arg(certificate_path)
        .arg("-config")
        .arg(file.path())
        .status()
        .context("Failed to launch OpenSSL")?
        .code();

    if code != Some(0) {
        return Err(anyhow::anyhow!("OpenSSL exited unsuccessfully"));
    }

    let code = Command::new(&openssl)
        .arg("pkcs12")
        .arg("-export")
        .arg("-out")
        .arg(identity_path)
        .arg("-inkey")
        .arg(key_path)
        .arg("-in")
        .arg(certificate_path)
        .status()
        .context("Failed to launch OpenSSL")?
        .code();

    if code != Some(0) {
        return Err(anyhow::anyhow!("OpenSSL exited unsuccessfully"));
    }

    Ok(())
}

#[derive(StructOpt)]
#[structopt(
    name = "rkvm-certificate-gen",
    about = "A tool to generate certificates to use with rkvm"
)]
struct Args {
    #[structopt(help = "Path to output identity file (PKCS12 archive)")]
    identity_path: PathBuf,
    #[structopt(help = "Path to output certificate file (PEM file)")]
    certificate_path: PathBuf,
    #[structopt(help = "Path to output key file (PEM file)")]
    key_path: PathBuf,
    #[structopt(
        long,
        short,
        help = "List of DNS names to be used, can be empty if at least one IP address is provided"
    )]
    dns_names: Vec<String>,
    #[structopt(
        long,
        short,
        help = "List of IP addresses to be used, can be empty if at least one DNS name is provided"
    )]
    ip_addresses: Vec<IpAddr>,
}

fn main() {
    let args = Args::from_args();
    if let Err(err) = run(
        &args.identity_path,
        &args.certificate_path,
        &args.key_path,
        &args.dns_names,
        &args.ip_addresses,
    ) {
        println!("Error: {}", err);
        process::exit(1);
    }
}
