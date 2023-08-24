use socket2::{SockRef, TcpKeepalive};
use std::io::Error;
use std::time::Duration;
use tokio::net::TcpStream;

pub fn configure(stream: &TcpStream) -> Result<(), Error> {
    stream.set_linger(None)?;
    stream.set_nodelay(false)?;

    SockRef::from(&stream).set_tcp_keepalive(
        &TcpKeepalive::new()
            .with_time(Duration::from_secs(1))
            .with_interval(Duration::from_secs(10))
            .with_retries(1),
    )?;

    Ok(())
}
