mod message;
mod version;

use std::io::{Error, ErrorKind};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use version::Version;

pub use message::Message;

pub async fn negotiate<T: AsyncRead + AsyncWrite + Send + Unpin>(
    stream: &mut T,
) -> Result<(), Error> {
    #[derive(Error, Debug)]
    #[error("Invalid version (expected {expected}, got {got} instead)")]
    struct InvalidVersionError {
        expected: Version,
        got: Version,
    }

    Version::CURRENT.encode(stream).await?;
    stream.flush().await?;

    let version = Version::decode(stream).await?;
    if version != Version::CURRENT {
        return Err(Error::new(
            ErrorKind::InvalidData,
            InvalidVersionError {
                expected: Version::CURRENT,
                got: version,
            },
        ));
    }

    Ok(())
}
