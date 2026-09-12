use crate::interceptor::InterceptorPlatform;
use std::io::Error;

#[cfg(target_os = "windows")]
pub use {crate::windows::monitor::MonitorWindows as Monitor};
#[cfg(target_os = "linux")]
pub use {crate::linux::monitor::MonitorLinux as Monitor};

pub trait MonitorPlatform: Sized {
    type Interceptor: InterceptorPlatform;

    fn new() -> Self;

    fn read<'a>(&'a mut self) -> impl std::future::Future<Output = Result<Self::Interceptor, Error>> +Send + 'a;
}