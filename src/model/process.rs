use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
}

pub fn enumerate_processes() -> Result<Vec<ProcessEntry>, String> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err("CreateToolhelp32Snapshot failed".to_owned());
    }

    let mut processes = Vec::new();
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let first_ok = unsafe { Process32FirstW(snapshot, &mut entry as *mut PROCESSENTRY32W) } != 0;
    if first_ok {
        loop {
            processes.push(ProcessEntry {
                pid: entry.th32ProcessID,
                name: utf16_to_string(&entry.szExeFile),
            });

            let next_ok = unsafe { Process32NextW(snapshot, &mut entry as *mut PROCESSENTRY32W) } != 0;
            if !next_ok {
                break;
            }
        }
    }

    unsafe {
        CloseHandle(snapshot);
    }

    processes.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.pid.cmp(&b.pid)));
    Ok(processes)
}

fn utf16_to_string(input: &[u16]) -> String {
    let len = input.iter().position(|ch| *ch == 0).unwrap_or(input.len());
    String::from_utf16_lossy(&input[..len])
}
