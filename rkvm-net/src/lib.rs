use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::io::{Error, ErrorKind};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

// Is it bold to assume there won't be more than 65536 protocol versions?
pub const PROTOCOL_VERSION: u16 = 2;

pub async fn read_version<R>(mut reader: R) -> Result<u16, Error>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes).await?;

    Ok(u16::from_le_bytes(bytes))
}

pub async fn write_version<W>(mut writer: W, version: u16) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    writer.write_all(&version.to_le_bytes()).await
}

pub async fn read_message<R, T>(mut reader: R) -> Result<T, Error>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let length = {
        let mut bytes = [0; 1];
        reader.read_exact(&mut bytes).await?;

        bytes[0]
    };

    let mut data = vec![0; length as usize];
    reader.read_exact(&mut data).await?;

    bincode::deserialize(&data).map_err(|err| Error::new(ErrorKind::InvalidData, err))
}

pub async fn write_message<W, T: Serialize>(mut writer: W, message: &T) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    let data =
        bincode::serialize(&message).map_err(|err| Error::new(ErrorKind::InvalidInput, err))?;
    let length: u8 = data
        .len()
        .try_into()
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "Serialized data is too large"))?;
    writer.write_all(&length.to_le_bytes()).await?;
    writer.write_all(&data).await?;

    Ok(())
}
