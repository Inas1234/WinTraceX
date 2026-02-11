mod model {
    pub mod event;
    pub mod ipc;
}

use model::event::Event;
use model::ipc::TRACE_UDP_BIND_ADDR;
use retour::GenericDetour;
use std::ffi::c_void;
use std::net::UdpSocket;
use std::sync::OnceLock;
use std::time::Instant;
use windows_sys::Win32::Foundation::{CloseHandle, HINSTANCE, RECT};
use windows_sys::Win32::Graphics::Gdi::DEVMODEW;
use windows_sys::Win32::System::LibraryLoader::{
    DisableThreadLibraryCalls, GetProcAddress, LoadLibraryA,
};
use windows_sys::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows_sys::Win32::System::Threading::{
    CreateThread, GetCurrentProcessId, GetCurrentThreadId,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{AdjustWindowRectEx, WS_OVERLAPPEDWINDOW};

type FnCreateWindowExW = unsafe extern "system" fn(
    u32,
    *const u16,
    *const u16,
    u32,
    i32,
    i32,
    i32,
    i32,
    isize,
    isize,
    isize,
    *const c_void,
) -> isize;
type FnSetWindowPos = unsafe extern "system" fn(isize, isize, i32, i32, i32, i32, u32) -> i32;
type FnMoveWindow = unsafe extern "system" fn(isize, i32, i32, i32, i32, i32) -> i32;
type FnChangeDisplaySettingsExW =
    unsafe extern "system" fn(*const u16, *const DEVMODEW, isize, u32, *const c_void) -> i32;
type FnAdjustWindowRectEx = unsafe extern "system" fn(*mut RECT, u32, i32, u32) -> i32;

static START_TIME: OnceLock<Instant> = OnceLock::new();
static UDP_SOCKET: OnceLock<Option<UdpSocket>> = OnceLock::new();

static CREATE_WINDOW_EXW_HOOK: OnceLock<GenericDetour<FnCreateWindowExW>> = OnceLock::new();
static SET_WINDOW_POS_HOOK: OnceLock<GenericDetour<FnSetWindowPos>> = OnceLock::new();
static MOVE_WINDOW_HOOK: OnceLock<GenericDetour<FnMoveWindow>> = OnceLock::new();
static CHANGE_DISPLAY_SETTINGS_EXW_HOOK: OnceLock<GenericDetour<FnChangeDisplaySettingsExW>> =
    OnceLock::new();
static ADJUST_WINDOW_RECT_EX_HOOK: OnceLock<GenericDetour<FnAdjustWindowRectEx>> = OnceLock::new();

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(module: HINSTANCE, reason: u32, _reserved: *mut c_void) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        unsafe {
            DisableThreadLibraryCalls(module);
        }

        let thread = unsafe {
            CreateThread(
                std::ptr::null(),
                0,
                Some(agent_init_thread),
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
            )
        };

        if !thread.is_null() {
            unsafe {
                CloseHandle(thread);
            }
        }
    }

    1
}

unsafe extern "system" fn agent_init_thread(_param: *mut c_void) -> u32 {
    let _ = install_hooks();
    trigger_smoke_test_call();
    0
}

fn install_hooks() -> Result<(), String> {
    let create_target: FnCreateWindowExW = unsafe { resolve_user32_proc(b"CreateWindowExW\0")? };
    let set_window_pos_target: FnSetWindowPos = unsafe { resolve_user32_proc(b"SetWindowPos\0")? };
    let move_window_target: FnMoveWindow = unsafe { resolve_user32_proc(b"MoveWindow\0")? };
    let change_display_target: FnChangeDisplaySettingsExW =
        unsafe { resolve_user32_proc(b"ChangeDisplaySettingsExW\0")? };
    let adjust_rect_target: FnAdjustWindowRectEx =
        unsafe { resolve_user32_proc(b"AdjustWindowRectEx\0")? };

    let create_hook = unsafe { GenericDetour::new(create_target, create_window_exw_detour) }
        .map_err(|e| format!("CreateWindowExW init failed: {e}"))?;
    let set_window_pos_hook = unsafe { GenericDetour::new(set_window_pos_target, set_window_pos_detour) }
        .map_err(|e| format!("SetWindowPos init failed: {e}"))?;
    let move_window_hook = unsafe { GenericDetour::new(move_window_target, move_window_detour) }
        .map_err(|e| format!("MoveWindow init failed: {e}"))?;
    let change_display_hook =
        unsafe { GenericDetour::new(change_display_target, change_display_settings_exw_detour) }
            .map_err(|e| format!("ChangeDisplaySettingsExW init failed: {e}"))?;
    let adjust_rect_hook = unsafe { GenericDetour::new(adjust_rect_target, adjust_window_rect_ex_detour) }
        .map_err(|e| format!("AdjustWindowRectEx init failed: {e}"))?;

    if CREATE_WINDOW_EXW_HOOK.get().is_none() {
        CREATE_WINDOW_EXW_HOOK
            .set(create_hook)
            .map_err(|_| "CreateWindowExW hook was already set".to_owned())?;
        SET_WINDOW_POS_HOOK
            .set(set_window_pos_hook)
            .map_err(|_| "SetWindowPos hook was already set".to_owned())?;
        MOVE_WINDOW_HOOK
            .set(move_window_hook)
            .map_err(|_| "MoveWindow hook was already set".to_owned())?;
        CHANGE_DISPLAY_SETTINGS_EXW_HOOK
            .set(change_display_hook)
            .map_err(|_| "ChangeDisplaySettingsExW hook was already set".to_owned())?;
        ADJUST_WINDOW_RECT_EX_HOOK
            .set(adjust_rect_hook)
            .map_err(|_| "AdjustWindowRectEx hook was already set".to_owned())?;

        unsafe {
            CREATE_WINDOW_EXW_HOOK
                .get()
                .expect("CreateWindowExW hook missing after set")
                .enable()
        }
        .map_err(|e| format!("CreateWindowExW enable failed: {e}"))?;
        unsafe {
            SET_WINDOW_POS_HOOK
                .get()
                .expect("SetWindowPos hook missing after set")
                .enable()
        }
        .map_err(|e| format!("SetWindowPos enable failed: {e}"))?;
        unsafe {
            MOVE_WINDOW_HOOK
                .get()
                .expect("MoveWindow hook missing after set")
                .enable()
        }
        .map_err(|e| format!("MoveWindow enable failed: {e}"))?;
        unsafe {
            CHANGE_DISPLAY_SETTINGS_EXW_HOOK
                .get()
                .expect("ChangeDisplaySettingsExW hook missing after set")
                .enable()
        }
        .map_err(|e| format!("ChangeDisplaySettingsExW enable failed: {e}"))?;
        unsafe {
            ADJUST_WINDOW_RECT_EX_HOOK
                .get()
                .expect("AdjustWindowRectEx hook missing after set")
                .enable()
        }
        .map_err(|e| format!("AdjustWindowRectEx enable failed: {e}"))?;
    }

    Ok(())
}

fn trigger_smoke_test_call() {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 1280,
        bottom: 720,
    };

    unsafe {
        let _ = AdjustWindowRectEx(&mut rect as *mut RECT, WS_OVERLAPPEDWINDOW, 0, 0);
    }
}

unsafe extern "system" fn create_window_exw_detour(
    ex_style: u32,
    class_name: *const u16,
    window_name: *const u16,
    style: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    parent: isize,
    menu: isize,
    instance: isize,
    param: *const c_void,
) -> isize {
    let hwnd = unsafe {
        CREATE_WINDOW_EXW_HOOK
            .get()
            .expect("CreateWindowExW hook not installed")
            .call(
                ex_style,
                class_name,
                window_name,
                style,
                x,
                y,
                width,
                height,
                parent,
                menu,
                instance,
                param,
            )
    };

    send_event(make_event(
        "CreateWindowExW",
        format!(
            "x={x} y={y} width={width} height={height} style=0x{style:08X} ex=0x{ex_style:08X}"
        ),
        format!("HWND=0x{hwnd:016X}"),
    ));

    hwnd
}

unsafe extern "system" fn set_window_pos_detour(
    hwnd: isize,
    hwnd_insert_after: isize,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    flags: u32,
) -> i32 {
    let result = unsafe {
        SET_WINDOW_POS_HOOK
            .get()
            .expect("SetWindowPos hook not installed")
            .call(hwnd, hwnd_insert_after, x, y, cx, cy, flags)
    };
    send_event(make_event(
        "SetWindowPos",
        format!("hwnd=0x{hwnd:016X} x={x} y={y} w={cx} h={cy} flags=0x{flags:08X}"),
        bool_result(result),
    ));
    result
}

unsafe extern "system" fn move_window_detour(
    hwnd: isize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    repaint: i32,
) -> i32 {
    let result = unsafe {
        MOVE_WINDOW_HOOK
            .get()
            .expect("MoveWindow hook not installed")
            .call(hwnd, x, y, width, height, repaint)
    };
    send_event(make_event(
        "MoveWindow",
        format!("hwnd=0x{hwnd:016X} x={x} y={y} w={width} h={height} repaint={repaint}"),
        bool_result(result),
    ));
    result
}

unsafe extern "system" fn change_display_settings_exw_detour(
    device_name: *const u16,
    dev_mode: *const DEVMODEW,
    hwnd: isize,
    flags: u32,
    lparam: *const c_void,
) -> i32 {
    let result = unsafe {
        CHANGE_DISPLAY_SETTINGS_EXW_HOOK
            .get()
            .expect("ChangeDisplaySettingsExW hook not installed")
            .call(device_name, dev_mode, hwnd, flags, lparam)
    };
    send_event(make_event(
        "ChangeDisplaySettingsExW",
        format!("hwnd=0x{hwnd:016X} flags=0x{flags:08X} device_ptr={device_name:p}"),
        format!("DISP_CHANGE={result}"),
    ));
    result
}

unsafe extern "system" fn adjust_window_rect_ex_detour(
    rect: *mut RECT,
    style: u32,
    has_menu: i32,
    ex_style: u32,
) -> i32 {
    let result = unsafe {
        ADJUST_WINDOW_RECT_EX_HOOK
            .get()
            .expect("AdjustWindowRectEx hook not installed")
            .call(rect, style, has_menu, ex_style)
    };
    send_event(make_event(
        "AdjustWindowRectEx",
        format!("style=0x{style:08X} ex=0x{ex_style:08X} has_menu={has_menu}"),
        bool_result(result),
    ));
    result
}

fn send_event(event: Event) {
    let Ok(payload) = serde_json::to_vec(&event) else {
        return;
    };

    let socket_option = UDP_SOCKET.get_or_init(|| UdpSocket::bind("127.0.0.1:0").ok());
    if let Some(socket) = socket_option.as_ref() {
        let _ = socket.send_to(&payload, TRACE_UDP_BIND_ADDR);
    }
}

fn make_event(api: &str, summary: String, result: String) -> Event {
    let thread_id = unsafe { GetCurrentThreadId() };
    let process_id = unsafe { GetCurrentProcessId() };

    Event {
        timestamp_ms: elapsed_ms(),
        api: api.to_owned(),
        summary,
        caller: format!("pid:{process_id} thread:{thread_id}"),
        thread_id,
        result,
    }
}

fn bool_result(value: i32) -> String {
    if value == 0 {
        "FALSE".to_owned()
    } else {
        "TRUE".to_owned()
    }
}

fn elapsed_ms() -> u64 {
    let started_at = START_TIME.get_or_init(Instant::now);
    let millis = started_at.elapsed().as_millis();
    millis.min(u64::MAX as u128) as u64
}

unsafe fn resolve_user32_proc<T>(name: &'static [u8]) -> Result<T, String> {
    let module = unsafe { LoadLibraryA(b"user32.dll\0".as_ptr()) };
    if module.is_null() {
        return Err("LoadLibraryA(user32.dll) failed".to_owned());
    }

    let proc = unsafe { GetProcAddress(module, name.as_ptr()) };
    let proc = proc.ok_or_else(|| format!("GetProcAddress failed for {}", display_proc(name)))?;

    Ok(proc_to_fn(proc))
}

fn proc_to_fn<T>(proc: unsafe extern "system" fn() -> isize) -> T {
    let raw = proc as *const ();
    unsafe { std::mem::transmute_copy(&raw) }
}

fn display_proc(name: &[u8]) -> String {
    let end = name.iter().position(|byte| *byte == 0).unwrap_or(name.len());
    String::from_utf8_lossy(&name[..end]).to_string()
}
