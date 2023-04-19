use clap::Parser;
use std::env;
use std::fmt::{self, Write as _};
use std::io::{self, Write as _};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus};
use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Parser)]
#[clap(
    name = "rkvm-certificate-gen",
    about = "A tool to generate certificates to use with rkvm"
)]
struct Args {
    #[clap(help = "Path to output certificate file (PEM file)")]
    certificate: PathBuf,
    #[structopt(help = "Path to output key file (PEM file)")]
    key: PathBuf,
    #[clap(
        long,
        short,
        help = "List of DNS names to be used, can be empty if at least one IP address is provided"
    )]
    dns_names: Vec<String>,
    #[clap(
        long,
        short,
        help = "List of IP addresses to be used, can be empty if at least one DNS name is provided"
    )]
    ip_addresses: Vec<IpAddr>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    if args.dns_names.is_empty() && args.ip_addresses.is_empty() {
        eprintln!("No DNS names or IP addresses were provided");
        return ExitCode::FAILURE;
    }

    let result = run(
        &args.certificate,
        &args.key,
        &args.dns_names,
        &args.ip_addresses,
    );

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {}", err);
            ExitCode::FAILURE
        }
    }
}

#[derive(Error, Debug)]
enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Fmt(#[from] fmt::Error),
    #[error("Bad exit: {0}")]
    BadExit(ExitStatus),
}

fn run(
    certificate: &Path,
    key: &Path,
    dns_names: &[String],
    ip_addresses: &[IpAddr],
) -> Result<(), Error> {
    let mut config = "[req]
prompt = no
default_bits = 2048
distinguished_name = req_distinguished_name
req_extensions = req_ext
x509_extensions = v3_req
[req_distinguished_name]
commonName = rkvm
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

    let mut file = NamedTempFile::new()?;
    file.write_all(config.as_bytes())?;

    let openssl = env::var_os("OPENSSL").unwrap_or_else(|| "openssl".to_owned().into());
    let status = Command::new(&openssl)
        .arg("req")
        .arg("-sha256")
        .arg("-x509")
        .arg("-nodes")
        .arg("-newkey")
        .arg("rsa:2048")
        .arg("-keyout")
        .arg(key)
        .arg("-out")
        .arg(certificate)
        .arg("-config")
        .arg(file.path())
        .status()?;

    if !status.success() {
        return Err(Error::BadExit(status));
    }

    Ok(())
}
