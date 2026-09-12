#![cfg(target_os="windows")]
mod client;
mod config;
mod connection;
mod windows_daemon;

use crate::client::{init_tracing, Error, RkvmWriter};
use crate::windows_daemon::{writer::ClientWriter, client_process::ClientProcess, stream::LockWriter};

use rkvm_input::windows::writer::WriterWindows;
use rkvm_net::Update;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use std::thread::sleep;
use tokio::io::{WriteHalf, split};
use tokio::net::windows::named_pipe::NamedPipeServer;
use tokio::sync::mpsc::{channel, Receiver};
use tokio::time::interval;
use tracing::Instrument;
use windows_service::define_windows_service;
use windows_service::service::{ServiceControl,ServiceControlAccept,ServiceExitCode,ServiceState,ServiceStatus,ServiceType};
use windows_service::service_control_handler::{register,ServiceStatusHandle,ServiceControlHandlerResult};
use windows_service::service_dispatcher;

const SERVICE_NAME: &str = "RkvmService";
const SERVICE_LOG: &str = r"C:\ProgramData\rkvm\rkvm-service.log";
const SERVICE_CFG: &str = r"C:\ProgramData\rkvm\client.toml";

static CHECKPOINT: AtomicU32 = AtomicU32::new(0);
const WAIT_HINT: Duration = Duration::from_secs(30);

enum ServiceEvent {
    Stop,
    Restart,
}

define_windows_service!(ffi_service_main, service_main);

fn set_service_state(handle: &ServiceStatusHandle, state: ServiceState, exit_code: u32) -> windows_service::Result<()> {
    let controls = match state {
        ServiceState::Running => ServiceControlAccept::STOP | ServiceControlAccept::SESSION_CHANGE,
        _ => ServiceControlAccept::empty()
    };
    let checkpoint = match state {
        ServiceState::StartPending => CHECKPOINT.fetch_add(1, Ordering::SeqCst) + 1,
        _ => {
            CHECKPOINT.store(0, Ordering::SeqCst);
            0
        }
    };
    let wait_hint = match state {
        ServiceState::StartPending => WAIT_HINT,
        _ => Duration::default()
    };

    let status = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: controls,
        exit_code: ServiceExitCode::Win32(exit_code),
        checkpoint: checkpoint,
        wait_hint: wait_hint,
        process_id: None,
    };
    handle.set_service_status(status)
}

fn main() -> windows_service::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = service_run() {
        tracing::error!(error = ?e, "Service crashed");
    }
}
fn service_run() -> windows_service::Result<()> {
    let (tx, mut rx) = channel(4);
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                tracing::info!("Service stop requested");
                let _ = tx.try_send(ServiceEvent::Stop);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::SessionChange(change) => {
                tracing::info!("Session change: {:?}", change);
                let _ = tx.try_send(ServiceEvent::Restart);
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let handle = register(SERVICE_NAME, event_handler)?;
    set_service_state(&handle, ServiceState::StartPending, 0)?;
    init_tracing(&"info".to_string(), &Some(PathBuf::from(SERVICE_LOG)));
    tracing::info!("Starting service");

    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    loop {
        match rt.block_on(process(&handle,&mut rx)) {
            Ok(_) => {
                tracing::info!("Service stopped normally");
                set_service_state(&handle, ServiceState::Stopped, 0)?;
            }
            Err(e) => {
                tracing::error!(error = ?e, "Service crashed");
                set_service_state(&handle, ServiceState::Stopped, 1)?;
            }
        };
        sleep(Duration::from_secs(2));
    }
}

async fn process(handle: &ServiceStatusHandle, rx: &mut Receiver<ServiceEvent>) -> Result<(), Error> {
    tracing::info!("Service started");

    let mut cl:ClientProcess = ClientProcess::new().await?;

    let stream = connection::init_stream(SERVICE_CFG).await?;
    let (mut stream_r, mut stream_w) = split(stream);

    let srv_update = WriterWindows::new(ClientWriter::new(cl.writer()));

    let ping_w = cl.writer();
    let mut restart_w = cl.writer();
    let span_server = tracing::info_span!("run", stream="server");
    let span_client = tracing::info_span!("run", stream="client");
    set_service_state(&handle, ServiceState::Running, 0)?;
    tokio::select! {
        res = client::run(&mut stream_r, &mut stream_w, srv_update).instrument(span_server) => res,
        res = cl.run().instrument(span_client) => res,
        res = ping_sender(ping_w) => res,
        _ = async {
            loop {
                match rx.recv().await {
                    Some(ServiceEvent::Stop) => {
                        tracing::info!("Service requested stop");
                        break ();
                    }
                    Some(ServiceEvent::Restart) => {
                        tracing::info!("Service restarting");
                        let _ = restart_w.send(Update::Stop);
                    }
                    _ => {}
                }
            }
        } => Ok(())
    }?;
    let _ = stream_w.send(Update::Stop);
    Ok(())
}

async fn ping_sender(mut w: LockWriter<WriteHalf<NamedPipeServer>>) -> Result<(),Error> {
    let mut interval = interval(rkvm_net::PING_INTERVAL);
    interval.tick().await;
    loop {
        interval.tick().await;
        w.send(Update::Ping).await?
    }
}
