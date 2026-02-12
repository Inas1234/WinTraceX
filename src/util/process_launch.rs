use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, CreateProcessW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION, ResumeThread,
    STARTUPINFOW,
};

pub struct SuspendedProcess {
    pub pid: u32,
    process_handle: HANDLE,
    thread_handle: HANDLE,
}

impl SuspendedProcess {
    pub fn resume(&self) -> Result<(), String> {
        let result = unsafe { ResumeThread(self.thread_handle) };
        if result == u32::MAX {
            return Err(format!("ResumeThread failed (GetLastError={})", unsafe {
                GetLastError()
            }));
        }

        Ok(())
    }
}

impl Drop for SuspendedProcess {
    fn drop(&mut self) {
        unsafe {
            if !self.thread_handle.is_null() {
                CloseHandle(self.thread_handle);
            }
            if !self.process_handle.is_null() {
                CloseHandle(self.process_handle);
            }
        }
    }
}

pub fn launch_target_exe_suspended(path: &str) -> Result<SuspendedProcess, String> {
    let exe_path = validate_exe_path(path)?;
    let mut exe_wide: Vec<u16> = exe_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut startup_info: STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut process_info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let flags: PROCESS_CREATION_FLAGS = CREATE_SUSPENDED;

    let created = unsafe {
        CreateProcessW(
            exe_wide.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            flags,
            std::ptr::null(),
            std::ptr::null(),
            &startup_info as *const STARTUPINFOW,
            &mut process_info as *mut PROCESS_INFORMATION,
        )
    } != 0;

    if !created {
        return Err(format!("CreateProcessW failed (GetLastError={})", unsafe {
            GetLastError()
        }));
    }

    Ok(SuspendedProcess {
        pid: process_info.dwProcessId,
        process_handle: process_info.hProcess,
        thread_handle: process_info.hThread,
    })
}

fn validate_exe_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Enter an EXE path.".to_owned());
    }

    let exe_path = Path::new(trimmed);
    if !exe_path.exists() {
        return Err(format!("File does not exist: {trimmed}"));
    }

    if !exe_path.is_file() {
        return Err(format!("Path is not a file: {trimmed}"));
    }

    let is_exe = exe_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    if !is_exe {
        return Err("Target must be an .exe file.".to_owned());
    }

    Ok(exe_path.to_path_buf())
}
