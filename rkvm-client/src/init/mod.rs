#[cfg(target_os="linux")]
mod linux;
#[cfg(target_os="linux")]
pub use linux::*;

#[cfg(all(target_os="windows",feature="windows-service"))]
mod windows_service;
#[cfg(all(target_os="windows",feature="windows-service"))]
pub use windows_service::*;

#[cfg(all(target_os="windows",not(feature="windows-service")))]
mod windows;
#[cfg(all(target_os="windows",not(feature="windows-service")))]
pub use windows::*;

