use rkvm_input::windows::writer_simple::WriterWindowsSimple;
use crate::client::{self, Error};
use crate::LockWriter;
use std::ffi::{CStr, c_void};
use std::io::ErrorKind;
use std::ptr::{addr_of_mut, null_mut};
use tokio::io::{ReadHalf, WriteHalf, split};
use tokio::net::windows::named_pipe::{ServerOptions, NamedPipeServer};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
use windows::Win32::Security::{DuplicateTokenEx, GetTokenInformation, SecurityImpersonation, TokenLinkedToken, TokenPrimary, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_ALL_ACCESS, TOKEN_ASSIGN_PRIMARY, TOKEN_LINKED_TOKEN};
use windows::Win32::Security::Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS};
use windows::Win32::System::RemoteDesktop::{ProcessIdToSessionId, WTSGetActiveConsoleSessionId, WTSQueryUserToken};
use windows::Win32::System::Threading::{CreateProcessAsUserW, OpenProcess, OpenProcessToken, TerminateProcess, CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, PROCESS_ALL_ACCESS, PROCESS_INFORMATION, STARTUPINFOW};

const SERVICE_PIPE: &str = r"\\.\pipe\rkvm";
const CLIENT_PATH: &str = r"C:\ProgramData\rkvm\rkvm-client.exe";
const CLIENT_LOG: &str = r"C:\ProgramData\rkvm\client.log";


fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn winlogon_pid(session_id: u32) -> u32 {
    let handle = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
        Err(e) => {
            tracing::warn!("Failed to get process snapshot {:?}", e);
            return 0;
        },
        Ok(h) => h
    };
    let mut proc = PROCESSENTRY32::default();
    proc.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;
    if let Err(e) = Process32First(handle, &mut proc) {
       tracing::warn!("Failed to get first process {:?}", e);
       let _ = CloseHandle(handle);
       return 0;
    }
    let mut logon_sid:u32=0;
    let mut pid:u32=0;
    loop {
        let exe = CStr::from_ptr(proc.szExeFile.as_ptr()).to_string_lossy();
        if exe.eq_ignore_ascii_case("winlogon.exe") && ProcessIdToSessionId(proc.th32ProcessID, &mut logon_sid).is_ok() && logon_sid==session_id {
            pid=proc.th32ProcessID;
            break;
        }
        if let Err(e) = Process32Next(handle, &mut proc) {
            tracing::warn!("Failed to get next process {:?}", e);
            break;
        }
    }
    let _ = CloseHandle(handle);
    return pid;
}

unsafe fn usertoken() -> Result<HANDLE,Error> {
    let session_id = WTSGetActiveConsoleSessionId();

    let winlogon = winlogon_pid(session_id);
    tracing::info!("Found winlogon {}", winlogon);

    let mut token: HANDLE = HANDLE::default();
    match OpenProcess(PROCESS_ALL_ACCESS, false, winlogon) {
        Ok(handle) => {
            let r = OpenProcessToken(handle, TOKEN_ASSIGN_PRIMARY|TOKEN_ALL_ACCESS, &mut token);
            let _ = CloseHandle(handle);
            match r {
                Ok(_) => return Ok(token),
                Err(e) => tracing::warn!("Failed to Get winlogon token {:?}", e)
            }
        },
        Err(e) => tracing::warn!("Failed to open winlogon process {:?}", e)
    };
    WTSQueryUserToken(session_id, &mut token)?;
    
    let mut needed = 0 as u32;
    let _ = GetTokenInformation(token, TokenLinkedToken, None, 0, &mut needed);
    let mut buffer = vec![0u8; needed as usize];
    match GetTokenInformation(token, TokenLinkedToken, Some(buffer.as_mut_ptr() as *mut _), needed, &mut needed) {
        Ok(_) => {
            let token_linked: &TOKEN_LINKED_TOKEN = &*(buffer.as_ptr() as *const TOKEN_LINKED_TOKEN);
            token = token_linked.LinkedToken
        },
        Err(e) => tracing::info!("Failed to get linked token {:?}", e)
    };

    return Ok(token);
}

fn launch_client() -> Result<HANDLE,Error> {
    unsafe {
        let user_token = usertoken()?;

        let sa = SECURITY_ATTRIBUTES::default();
        let mut primary_token: HANDLE = HANDLE::default();
        DuplicateTokenEx(user_token, TOKEN_ASSIGN_PRIMARY|TOKEN_ALL_ACCESS, Some(&sa), SecurityImpersonation, TokenPrimary, &mut primary_token)?;
        let _ = CloseHandle(user_token);

        let mut si = STARTUPINFOW::default();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

        let mut pi = PROCESS_INFORMATION::default();
        let mut cmd = to_wide(format!("\"{}\" --log-file {} {}", CLIENT_PATH, CLIENT_LOG, SERVICE_PIPE).as_str());
        CreateProcessAsUserW(Some(primary_token), PCWSTR::null(), Some(PWSTR(cmd.as_mut_ptr())), Some(&sa), None, false, CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT, None, PCWSTR::null(), &si, &mut pi)?;

        tracing::info!("client started with pid {}", pi.dwProcessId);
        let _ = CloseHandle(pi.hThread);
        let _ = CloseHandle(primary_token);
        Ok(pi.hProcess)
    }
}

fn new_pipe(first: bool) -> Result<NamedPipeServer, Error> {
    unsafe {
        let sddl = to_wide("D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;AU)");

        let mut security_descriptor: PSECURITY_DESCRIPTOR = PSECURITY_DESCRIPTOR(null_mut());

        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut security_descriptor,
            None
        )?;

        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security_descriptor.0,
            bInheritHandle: false.into(),
        };
        
        let res = ServerOptions::new().first_pipe_instance(first).write_dac(true).create_with_security_attributes_raw(SERVICE_PIPE, addr_of_mut!(sa) as *mut c_void);
        if res.is_err() {
            tracing::warn!("Failed to create pipe {:?}", res);
        }
        LocalFree(Some(HLOCAL(security_descriptor.0)));
        let pipe = res?;
        tracing::info!("Created pipe");
        Ok(pipe)
    }
}

pub struct ClientProcess {
    handle: HANDLE,
    reader: ReadHalf<NamedPipeServer>,
    writer: LockWriter<WriteHalf<NamedPipeServer>>,
}

impl Drop for ClientProcess {
    fn drop(self: &mut Self) {
        self.stop();
    }
}

impl ClientProcess {
    pub async fn new() -> Result<Self,Error> {

        let pipe = new_pipe(true)?;
        let connect = pipe.connect();
        let handle = launch_client()?;
        connect.await?;

        let (pipe_r, pipe_w) = split(pipe);
        Ok(ClientProcess{ handle: handle, reader: pipe_r, writer: LockWriter::new(pipe_w) })
    }

    pub fn writer(self: &Self) -> LockWriter<WriteHalf<NamedPipeServer>> {
        self.writer.clone()
    }

    pub async fn run(self: &mut Self) -> Result<(), Error> {
        loop {
            let res = client::run(&mut self.reader, &mut self.writer, WriterWindowsSimple::new()).await;
            if let Err(ref e) = res {
                tracing::info!("Client error: {:?}", e);
                match e {
                    Error::Network(ref io) => {
                        if io.kind() == ErrorKind::UnexpectedEof {
                            self.restart().await?;
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            return res;
        }
    }

    pub fn stop(self: &Self) {
        unsafe {
            let _ = TerminateProcess(self.handle, 0);
            let _ = CloseHandle(self.handle);
        }
    }

    pub async fn restart(self: &mut Self) -> Result<(),Error> {
        let mut w = self.writer.lock().await;
        self.stop();
        let pipe = new_pipe(false)?;
        let connect = pipe.connect();
        let handle = launch_client()?;
        connect.await?;

        let (pipe_r, pipe_w) = split(pipe);
        self.handle = handle;
        self.reader = pipe_r;
        *w = pipe_w;
        Ok(())
    }
}
