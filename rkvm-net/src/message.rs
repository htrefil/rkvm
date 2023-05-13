use bincode::{DefaultOptions, Options};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

#[async_trait::async_trait]
pub trait Message: Sized {
    async fn decode<R: AsyncRead + Send + Unpin>(stream: &mut R) -> Result<Self, Error>;

    async fn encode<W: AsyncWrite + Send + Unpin>(&self, stream: &mut W) -> Result<(), Error>;
}

#[async_trait::async_trait]
impl<T: DeserializeOwned + Serialize + Sync> Message for T {
    async fn decode<R: AsyncRead + Send + Unpin>(stream: &mut R) -> Result<Self, Error> {
        let length = stream.read_u16().await?;

        let mut data = vec![0; length.into()];
        stream.read_exact(&mut data).await?;

        let data = options()
            .deserialize(&data)
            .map_err(|err| Error::new(ErrorKind::InvalidData, err))?;

        log::trace!("Read {} bytes", 2 + length);

        Ok(data)
    }

    async fn encode<W: AsyncWrite + Send + Unpin>(&self, stream: &mut W) -> Result<(), Error> {
        let data = options()
            .serialize(self)
            .map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;

        let length = data
            .len()
            .try_into()
            .map_err(|_| Error::new(ErrorKind::InvalidInput, "Data too large"))?;

        stream.write_u16(length).await?;
        stream.write_all(&data).await?;

        log::trace!("Wrote {} bytes", 2 + data.len());

        Ok(())
    }
}

fn options() -> impl Options {
    DefaultOptions::new().with_limit(u16::MAX.into())
}
