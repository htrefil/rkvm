use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{Poll, PollOpt, Ready, Token};
use std::convert::AsRef;
use std::fs::{File, OpenOptions};
use std::io::{Error, Read, Write};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::pin::Pin;
use std::task::{self, Context};
use tokio::io::{AsyncRead, AsyncWrite, PollEvented};

pub struct AsyncFile {
    file: PollEvented<Inner>,
}

impl AsyncFile {
    pub async fn open(path: impl AsRef<Path>, mode: OpenMode) -> Result<Self, Error> {
        let mut options = OpenOptions::new();
        match mode {
            OpenMode::Read => options.read(true),
            OpenMode::Write => options.write(true),
            OpenMode::ReadWrite => options.read(true).write(true),
        };
        let path = path.as_ref().to_owned();
        let inner = Inner(tokio::task::spawn_blocking(move || options.open(path)).await??);

        Ok(Self {
            file: PollEvented::new(inner)?,
        })
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

struct Inner(File);

impl Evented for Inner {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), Error> {
        EventedFd(&self.0.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), Error> {
        EventedFd(&self.0.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> Result<(), Error> {
        EventedFd(&self.0.as_raw_fd()).deregister(poll)
    }
}

impl Read for Inner {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        self.0.read(buffer)
    }
}

impl Write for Inner {
    fn write(&mut self, data: &[u8]) -> Result<usize, Error> {
        self.0.write(data)
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.0.flush()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum OpenMode {
    Read,
    Write,
    ReadWrite,
}
