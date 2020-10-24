use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{Poll, PollOpt, Ready, Token};
use std::convert::AsRef;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{Error, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{self, Context};
use tokio::io::{AsyncRead, AsyncWrite, PollEvented};

pub struct AsyncFile {
    file: PollEvented<Inner>,
}

impl AsyncFile {
    pub async fn open(path: impl AsRef<Path>, mode: OpenMode) -> Result<Self, Error> {
        let path = path.as_ref().to_owned();

        tokio::task::spawn_blocking(move || Self::open_sync(path, mode)).await?
    }

    fn open_sync(path: PathBuf, mode: OpenMode) -> Result<Self, Error> {
        let mut options = OpenOptions::new();
        match mode {
            OpenMode::Read => options.read(true),
            OpenMode::Write => options.write(true),
        };

        let file = options.custom_flags(libc::O_NONBLOCK).open(&path)?;
        Ok(Self {
            file: PollEvented::new(Inner { file })?,
        })
    }
}

impl AsRawFd for AsyncFile {
    fn as_raw_fd(&self) -> RawFd {
        self.file.get_ref().file.as_raw_fd()
    }
}

impl AsyncRead for AsyncFile {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context,
        buffer: &mut [u8],
    ) -> task::Poll<Result<usize, Error>> {
        Pin::new(&mut self.file).poll_read(context, buffer)
    }
}

impl AsyncWrite for AsyncFile {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context,
        data: &[u8],
    ) -> task::Poll<Result<usize, Error>> {
        Pin::new(&mut self.file).poll_write(context, data)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context,
    ) -> task::Poll<Result<(), Error>> {
        Pin::new(&mut self.file).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context,
    ) -> task::Poll<Result<(), Error>> {
        Pin::new(&mut self.file).poll_shutdown(context)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum OpenMode {
    Read,
    Write,
}

struct Inner {
    file: File,
}

impl Read for Inner {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        self.file.read(buffer)
    }
}

impl Write for Inner {
    fn write(&mut self, data: &[u8]) -> Result<usize, Error> {
        self.file.write(data)
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.file.flush()
    }
}

impl Evented for Inner {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), Error> {
        EventedFd(&self.file.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), Error> {
        EventedFd(&self.file.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> Result<(), Error> {
        EventedFd(&self.file.as_raw_fd()).deregister(poll)
    }
}
