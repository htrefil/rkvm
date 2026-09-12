use std::sync::{LazyLock};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use windows::{
    core,
    Win32::{
        System::StationsAndDesktops::{OpenInputDesktop, SetThreadDesktop, CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP, DESKTOP_READOBJECTS, DESKTOP_ACCESS_FLAGS},
        Foundation::{GetLastError, GENERIC_WRITE},
        UI::Input::KeyboardAndMouse::{INPUT, SendInput},
    }
};

static TX:LazyLock<Sender<INPUT>> = LazyLock::new(|| create());

pub fn send_input(input:INPUT) {
    TX.send(input).unwrap()
}

fn create() -> Sender<INPUT> {
    let (tx, rx) = channel();
    spawn(rx);
    tx
}

fn spawn(rx: Receiver<INPUT>) -> thread::JoinHandle<Result<(), core::Error>> {
    thread::spawn(move || {
        unsafe {
            match OpenInputDesktop(DESKTOP_CONTROL_FLAGS::default(), false, DESKTOP_ACCESS_FLAGS(DESKTOP_SWITCHDESKTOP.0 | DESKTOP_READOBJECTS.0 | GENERIC_WRITE.0)) {
                Ok(h) => {
                    if let Err(e) = SetThreadDesktop(h) {
                        tracing::warn!("Failed to set thread desktop: {:?}", e);
                    }
                    let _ = CloseDesktop(h);
                },
                Err(e) => tracing::warn!("Failed to get current desktop {:?}", e)
            }
        }
        loop {
            match rx.recv() {
                Ok(input) => {
                    unsafe {
                        let n = SendInput(&[input], size_of::<INPUT>() as i32);
                        if n == 0 {
                            tracing::warn!("SendInput fail {:?}", GetLastError());
                            return Ok(());
                        }
                    }
                },
                Err(e) => {
                    tracing::warn!("Injector error {}", e);
                }
            }
        }
    })
}
