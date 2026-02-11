use std::cmp::max;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, ERROR_BAD_LENGTH,
    ERROR_PARTIAL_COPY,
};
use windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32,
};
use windows_sys::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows_sys::Win32::System::SystemInformation::{
    IMAGE_FILE_MACHINE, IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_ARM64,
    IMAGE_FILE_MACHINE_I386, IMAGE_FILE_MACHINE_UNKNOWN,
};
use windows_sys::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, IsWow64Process, IsWow64Process2, OpenProcess,
    WaitForSingleObject, INFINITE, LPTHREAD_START_ROUTINE, PROCESS_CREATE_THREAD,
    PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

pub fn inject_agent_dll(pid: u32) -> Result<String, String> {
    unsafe {
        let process = OpenProcess(
            PROCESS_CREATE_THREAD
                | PROCESS_QUERY_INFORMATION
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
            0,
            pid,
        );
        if process.is_null() {
            return Err(format!(
                "OpenProcess failed for PID {pid} (GetLastError={})",
                last_error_code()
            ));
        }
        let process_handle = HandleGuard(process);

        let target_machine = detect_process_machine(process_handle.0)?;
        let dll_path = resolve_agent_dll_path(target_machine)?;
        if target_machine == IMAGE_FILE_MACHINE_I386 {
            ensure_x86_runtime_dependencies(&dll_path)?;
        }
        let load_library_w_remote = resolve_remote_loadlibraryw(pid, target_machine)?;

        let dll_wide: Vec<u16> = dll_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let dll_len_bytes = dll_wide.len() * std::mem::size_of::<u16>();

        let remote_buffer = VirtualAllocEx(
            process_handle.0,
            std::ptr::null(),
            dll_len_bytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if remote_buffer.is_null() {
            return Err(format!(
                "VirtualAllocEx failed (GetLastError={})",
                last_error_code()
            ));
        }

        let mut bytes_written = 0usize;
        let write_ok = WriteProcessMemory(
            process_handle.0,
            remote_buffer,
            dll_wide.as_ptr() as *const _,
            dll_len_bytes,
            &mut bytes_written as *mut usize,
        ) != 0;
        if !write_ok || bytes_written != dll_len_bytes {
            let _ = VirtualFreeEx(process_handle.0, remote_buffer, 0, MEM_RELEASE);
            return Err(format!(
                "WriteProcessMemory failed (GetLastError={})",
                last_error_code()
            ));
        }

        let thread_start: LPTHREAD_START_ROUTINE =
            Some(std::mem::transmute(load_library_w_remote as usize));

        let remote_thread = CreateRemoteThread(
            process_handle.0,
            std::ptr::null(),
            0,
            thread_start,
            remote_buffer,
            0,
            std::ptr::null_mut(),
        );
        if remote_thread.is_null() {
            let _ = VirtualFreeEx(process_handle.0, remote_buffer, 0, MEM_RELEASE);
            return Err(format!(
                "CreateRemoteThread failed (GetLastError={})",
                last_error_code()
            ));
        }
        let thread_handle = HandleGuard(remote_thread);

        let wait_result = WaitForSingleObject(thread_handle.0, INFINITE);
        if wait_result != WAIT_OBJECT_0 {
            let _ = VirtualFreeEx(process_handle.0, remote_buffer, 0, MEM_RELEASE);
            return Err(format!("WaitForSingleObject failed with code {wait_result}"));
        }

        let mut remote_module_handle = 0u32;
        let exit_ok = GetExitCodeThread(thread_handle.0, &mut remote_module_handle as *mut u32) != 0;
        let _ = VirtualFreeEx(process_handle.0, remote_buffer, 0, MEM_RELEASE);

        if !exit_ok {
            return Err("GetExitCodeThread failed".to_owned());
        }

        if remote_module_handle == 0 {
            return Err(format!(
                "Remote LoadLibraryW returned NULL for {}. Common causes: missing DLL dependencies in target process or insufficient rights.",
                dll_path.display()
            ));
        }

        Ok(dll_path.display().to_string())
    }
}

fn resolve_agent_dll_path(target_machine: IMAGE_FILE_MACHINE) -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe().map_err(|e| format!("current_exe failed: {e}"))?;
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| "Failed to resolve current executable directory".to_owned())?;
    let target_dir = exe_dir
        .parent()
        .ok_or_else(|| "Failed to resolve target directory near current executable".to_owned())?;

    match target_machine {
        IMAGE_FILE_MACHINE_I386 => {
            let preferred = exe_dir.join("win_api_trace_agent_x86.dll");
            if preferred.exists() {
                return Ok(preferred);
            }

            let gnullvm_candidate = target_dir
                .join("i686-pc-windows-gnullvm")
                .join("debug")
                .join("win_api_trace_agent.dll");
            if gnullvm_candidate.exists() {
                return Ok(gnullvm_candidate);
            }

            let gnu_candidate = target_dir
                .join("i686-pc-windows-gnu")
                .join("debug")
                .join("win_api_trace_agent.dll");
            if gnu_candidate.exists() {
                return Ok(gnu_candidate);
            }

            Err(format!(
                "Target is x86 (I386), but no x86 agent DLL was found. Checked: `{}`, `{}`, `{}`. Build x86 agent with LLVM-MinGW: `winget install --id MartinStorsjo.LLVM-MinGW.UCRT --exact`, then in a shell set `PATH=<llvm-mingw-bin>;$PATH`, run `rustup target add i686-pc-windows-gnullvm`, then `cargo build --target i686-pc-windows-gnullvm --lib`.",
                preferred.display(),
                gnullvm_candidate.display(),
                gnu_candidate.display()
            ))
        }
        IMAGE_FILE_MACHINE_AMD64 | IMAGE_FILE_MACHINE_ARM64 | IMAGE_FILE_MACHINE_UNKNOWN => {
            let candidate = exe_dir.join("win_api_trace_agent.dll");
            if candidate.exists() {
                Ok(candidate)
            } else {
                Err(format!(
                    "Agent DLL not found for target architecture. Expected `{}`.",
                    candidate.display()
                ))
            }
        }
        _ => {
            let candidate = exe_dir.join("win_api_trace_agent.dll");
            if candidate.exists() {
                Ok(candidate)
            } else {
                Err(format!(
                    "Agent DLL not found for target machine {}. Expected `{}`.",
                    target_machine,
                    candidate.display()
                ))
            }
        }
    }
}

fn ensure_x86_runtime_dependencies(agent_dll_path: &Path) -> Result<(), String> {
    let output_dir = agent_dll_path
        .parent()
        .ok_or_else(|| format!("Invalid agent DLL path: {}", agent_dll_path.display()))?;

    let unwind_dst = output_dir.join("libunwind.dll");
    if unwind_dst.exists() {
        return Ok(());
    }

    let unwind_src = find_i686_libunwind()
        .ok_or_else(|| "Could not find i686 libunwind.dll. Install LLVM-MinGW UCRT and ensure i686 runtime is present.".to_owned())?;

    fs::copy(&unwind_src, &unwind_dst).map_err(|e| {
        format!(
            "Failed to copy {} to {}: {e}",
            unwind_src.display(),
            unwind_dst.display()
        )
    })?;

    Ok(())
}

fn find_i686_libunwind() -> Option<PathBuf> {
    // 1) Try locating from an available i686 toolchain in PATH.
    if let Some(path) = find_i686_libunwind_from_where() {
        return Some(path);
    }

    // 2) Try common winget install layout.
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let packages_dir = PathBuf::from(local_app_data).join("Microsoft\\WinGet\\Packages");
        if let Ok(package_entries) = fs::read_dir(packages_dir) {
            for package_entry in package_entries.flatten() {
                let package_name = package_entry.file_name().to_string_lossy().to_string();
                if !(package_name.starts_with("MartinStorsjo.LLVM-MinGW.UCRT_")
                    || package_name.starts_with("MartinStorsjo.LLVM-MinGW.MSVCRT_"))
                {
                    continue;
                }

                if let Ok(version_entries) = fs::read_dir(package_entry.path()) {
                    for version_entry in version_entries.flatten() {
                        let version_name = version_entry.file_name().to_string_lossy().to_string();
                        if !version_name.starts_with("llvm-mingw-") {
                            continue;
                        }

                        let candidate = version_entry
                            .path()
                            .join("i686-w64-mingw32")
                            .join("bin")
                            .join("libunwind.dll");
                        if candidate.exists() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
    }

    None
}

fn find_i686_libunwind_from_where() -> Option<PathBuf> {
    let output = Command::new("where")
        .arg("i686-w64-mingw32-gcc")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let gcc_path = PathBuf::from(line.trim());
        let root = gcc_path.parent()?.parent()?;
        let candidate = root
            .join("i686-w64-mingw32")
            .join("bin")
            .join("libunwind.dll");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

fn resolve_remote_loadlibraryw(pid: u32, target_machine: IMAGE_FILE_MACHINE) -> Result<usize, String> {
    let kernel32 = find_remote_module(pid, "kernel32.dll", target_machine)?;
    let load_library_rva = find_export_rva(&kernel32.path, "LoadLibraryW")? as usize;
    Ok(kernel32.base_addr + load_library_rva)
}

struct RemoteModule {
    base_addr: usize,
    path: PathBuf,
}

fn find_remote_module(
    pid: u32,
    wanted_module_name: &str,
    target_machine: IMAGE_FILE_MACHINE,
) -> Result<RemoteModule, String> {
    let flags_to_try: &[u32] = if target_machine == IMAGE_FILE_MACHINE_I386 {
        &[TH32CS_SNAPMODULE32, TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32]
    } else {
        &[TH32CS_SNAPMODULE, TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32]
    };

    let mut last_error = 0u32;

    for _attempt in 0..10 {
        for &flags in flags_to_try {
            let snapshot = unsafe { CreateToolhelp32Snapshot(flags, pid) };
            if snapshot == INVALID_HANDLE_VALUE {
                let error = last_error_code();
                last_error = error;
                continue;
            }

            let snapshot_handle = HandleGuard(snapshot);
            let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
            entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;

            let first_ok =
                unsafe { Module32FirstW(snapshot_handle.0, &mut entry as *mut MODULEENTRY32W) } != 0;
            if !first_ok {
                let error = last_error_code();
                last_error = error;
                continue;
            }

            loop {
                let module_name = utf16_to_string(&entry.szModule);
                if module_name.eq_ignore_ascii_case(wanted_module_name) {
                    let path = PathBuf::from(utf16_to_string(&entry.szExePath));
                    return Ok(RemoteModule {
                        base_addr: entry.modBaseAddr as usize,
                        path,
                    });
                }

                let next_ok =
                    unsafe { Module32NextW(snapshot_handle.0, &mut entry as *mut MODULEENTRY32W) } != 0;
                if !next_ok {
                    break;
                }
            }
        }

        // Target process may still be initializing loader metadata.
        if last_error == ERROR_PARTIAL_COPY || last_error == ERROR_BAD_LENGTH {
            thread::sleep(Duration::from_millis(60));
            continue;
        }

        break;
    }

    if last_error == ERROR_PARTIAL_COPY {
        return Err(format!(
            "CreateToolhelp32Snapshot(module) failed for PID {pid} (GetLastError={last_error}). The target may be protected or not fully initialized yet. Try again after the process is fully up, or run tracer/target with matching privileges."
        ));
    }

    Err(format!(
        "Failed to locate module {} in PID {} (last error={}).",
        wanted_module_name, pid, last_error
    ))
}

fn detect_process_machine(process: HANDLE) -> Result<IMAGE_FILE_MACHINE, String> {
    let mut process_machine: IMAGE_FILE_MACHINE = 0;
    let mut native_machine: IMAGE_FILE_MACHINE = 0;

    let wow64_2_ok = unsafe {
        IsWow64Process2(
            process,
            &mut process_machine as *mut IMAGE_FILE_MACHINE,
            &mut native_machine as *mut IMAGE_FILE_MACHINE,
        )
    } != 0;

    if wow64_2_ok {
        let machine = if process_machine == IMAGE_FILE_MACHINE_UNKNOWN {
            native_machine
        } else {
            process_machine
        };
        return Ok(machine);
    }

    let mut wow64 = 0i32;
    let wow64_ok = unsafe { IsWow64Process(process, &mut wow64 as *mut i32) } != 0;
    if !wow64_ok {
        return Err(format!(
            "IsWow64Process2/IsWow64Process failed (GetLastError={})",
            last_error_code()
        ));
    }

    #[cfg(target_arch = "x86_64")]
    {
        if wow64 != 0 {
            Ok(IMAGE_FILE_MACHINE_I386)
        } else {
            Ok(IMAGE_FILE_MACHINE_AMD64)
        }
    }

    #[cfg(target_arch = "x86")]
    {
        let _ = wow64;
        Ok(IMAGE_FILE_MACHINE_I386)
    }

    #[cfg(target_arch = "aarch64")]
    {
        if wow64 != 0 {
            Ok(IMAGE_FILE_MACHINE_I386)
        } else {
            Ok(IMAGE_FILE_MACHINE_ARM64)
        }
    }
}

fn find_export_rva(module_path: &Path, export_name: &str) -> Result<u32, String> {
    let data = std::fs::read(module_path)
        .map_err(|e| format!("Failed to read {}: {e}", module_path.display()))?;

    if read_u16(&data, 0)? != 0x5A4D {
        return Err(format!("{} is not a valid PE file", module_path.display()));
    }

    let nt_off = read_u32(&data, 0x3C)? as usize;
    if read_u32(&data, nt_off)? != 0x0000_4550 {
        return Err(format!("{} has invalid PE signature", module_path.display()));
    }

    let coff_off = nt_off + 4;
    let number_of_sections = read_u16(&data, coff_off + 2)? as usize;
    let size_of_optional_header = read_u16(&data, coff_off + 16)? as usize;
    let optional_off = coff_off + 20;

    let optional_magic = read_u16(&data, optional_off)?;
    let export_dir_off = match optional_magic {
        0x10B => optional_off + 96,
        0x20B => optional_off + 112,
        _ => {
            return Err(format!(
                "Unsupported optional header magic 0x{optional_magic:04X} for {}",
                module_path.display()
            ));
        }
    };

    let export_rva = read_u32(&data, export_dir_off)?;
    if export_rva == 0 {
        return Err(format!("{} has no export directory", module_path.display()));
    }

    let section_table_off = optional_off + size_of_optional_header;

    let rva_to_offset = |rva: u32| -> Result<usize, String> {
        for idx in 0..number_of_sections {
            let sec_off = section_table_off + idx * 40;
            let virtual_size = read_u32(&data, sec_off + 8)?;
            let virtual_address = read_u32(&data, sec_off + 12)?;
            let size_of_raw_data = read_u32(&data, sec_off + 16)?;
            let ptr_to_raw_data = read_u32(&data, sec_off + 20)?;

            let span = max(virtual_size, size_of_raw_data);
            let section_end = virtual_address.saturating_add(span);
            if rva >= virtual_address && rva < section_end {
                let delta = rva - virtual_address;
                let file_off = ptr_to_raw_data.saturating_add(delta) as usize;
                if file_off >= data.len() {
                    break;
                }
                return Ok(file_off);
            }
        }

        Err(format!(
            "Failed to map RVA 0x{rva:08X} in {}",
            module_path.display()
        ))
    };

    let export_off = rva_to_offset(export_rva)?;
    let number_of_names = read_u32(&data, export_off + 24)? as usize;
    let address_of_functions = read_u32(&data, export_off + 28)?;
    let address_of_names = read_u32(&data, export_off + 32)?;
    let address_of_name_ordinals = read_u32(&data, export_off + 36)?;

    let names_table_off = rva_to_offset(address_of_names)?;
    let ordinals_table_off = rva_to_offset(address_of_name_ordinals)?;
    let functions_table_off = rva_to_offset(address_of_functions)?;

    for idx in 0..number_of_names {
        let name_rva = read_u32(&data, names_table_off + idx * 4)?;
        let name_off = rva_to_offset(name_rva)?;
        let name = read_c_string(&data, name_off)?;

        if name == export_name {
            let ordinal = read_u16(&data, ordinals_table_off + idx * 2)? as usize;
            let func_rva = read_u32(&data, functions_table_off + ordinal * 4)?;
            return Ok(func_rva);
        }
    }

    Err(format!(
        "Export {} not found in {}",
        export_name,
        module_path.display()
    ))
}

fn read_u16(data: &[u8], off: usize) -> Result<u16, String> {
    let bytes = data
        .get(off..off + 2)
        .ok_or_else(|| format!("Out-of-bounds u16 read at offset {off}"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], off: usize) -> Result<u32, String> {
    let bytes = data
        .get(off..off + 4)
        .ok_or_else(|| format!("Out-of-bounds u32 read at offset {off}"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_c_string(data: &[u8], off: usize) -> Result<&str, String> {
    let tail = data
        .get(off..)
        .ok_or_else(|| format!("Out-of-bounds string read at offset {off}"))?;
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| format!("Unterminated string at offset {off}"))?;

    std::str::from_utf8(&tail[..end])
        .map_err(|e| format!("Invalid UTF-8 export name at offset {off}: {e}"))
}

fn utf16_to_string(input: &[u16]) -> String {
    let len = input.iter().position(|ch| *ch == 0).unwrap_or(input.len());
    String::from_utf16_lossy(&input[..len])
}

struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
                CloseHandle(self.0);
            }
        }
    }
}

fn last_error_code() -> u32 {
    unsafe { GetLastError() }
}
