use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub trait Message: Sized {
    async fn decode<R: AsyncRead + Send + Unpin>(stream: &mut R) -> Result<Self, Error>;

    async fn encode<W: AsyncWrite + Send + Unpin>(&self, stream: &mut W) -> Result<(), Error>;
}

impl<T: DeserializeOwned + Serialize + Sync> Message for T {
    async fn decode<R: AsyncRead + Send + Unpin>(stream: &mut R) -> Result<Self, Error> {
        let length = stream.read_u16().await?;

        let mut data = vec![0; length.into()];
        stream.read_exact(&mut data).await?;

        // postcard::from_bytes ignores trailing data, but a frame whose length prefix
        // overshoots the message is malformed, so reject the leftovers explicitly.
        let (data, rest) = postcard::take_from_bytes(&data)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;

        if !rest.is_empty() {
            return Err(Error::new(ErrorKind::InvalidData, "Trailing message data"));
        }

        tracing::trace!("Read {} bytes", 2 + length);

        Ok(data)
    }

    async fn encode<W: AsyncWrite + Send + Unpin>(&self, stream: &mut W) -> Result<(), Error> {
        let data =
            postcard::to_stdvec(self).map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;

        let length = data
            .len()
            .try_into()
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Data too large"))?;

        stream.write_u16(length).await?;
        stream.write_all(&data).await?;

        tracing::trace!("Wrote {} bytes", 2 + data.len());

        Ok(())
    }
}
