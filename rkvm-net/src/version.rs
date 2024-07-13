use crate::message::Message;

use std::fmt::{self, Display, Formatter};
use std::io::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Version(u16);

impl Version {
    pub const CURRENT: Self = Self(5);
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

impl Message for Version {
    async fn decode<R: AsyncRead + Send + Unpin>(stream: &mut R) -> Result<Self, Error> {
        stream.read_u16_le().await.map(Self)
    }

    async fn encode<W: AsyncWrite + Send + Unpin>(&self, stream: &mut W) -> Result<(), Error> {
        stream.write_u16_le(self.0).await
    }
}
