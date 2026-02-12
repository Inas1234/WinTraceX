use crate::model::event::Event;
use retour::GenericDetour;
use std::ffi::c_void;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::DEVMODEW;
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows_sys::Win32::UI::WindowsAndMessaging::{AdjustWindowRectEx, WS_OVERLAPPEDWINDOW};

pub mod injector;
pub mod udp_listener;

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
type FnDirectDrawCreate =
    unsafe extern "system" fn(*const c_void, *mut *mut c_void, *mut c_void) -> i32;
type FnDirectDrawCreateEx =
    unsafe extern "system" fn(*const c_void, *mut *mut c_void, *const c_void, *mut c_void) -> i32;
type FnDirect3DCreate9 = unsafe extern "system" fn(u32) -> *mut c_void;
type FnDirect3DCreate9Ex = unsafe extern "system" fn(u32, *mut *mut c_void) -> i32;

static EVENT_SENDER: OnceLock<Mutex<Option<Sender<Event>>>> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();

static CREATE_WINDOW_EXW_HOOK: OnceLock<GenericDetour<FnCreateWindowExW>> = OnceLock::new();
static SET_WINDOW_POS_HOOK: OnceLock<GenericDetour<FnSetWindowPos>> = OnceLock::new();
static MOVE_WINDOW_HOOK: OnceLock<GenericDetour<FnMoveWindow>> = OnceLock::new();
static CHANGE_DISPLAY_SETTINGS_EXW_HOOK: OnceLock<GenericDetour<FnChangeDisplaySettingsExW>> =
    OnceLock::new();
static ADJUST_WINDOW_RECT_EX_HOOK: OnceLock<GenericDetour<FnAdjustWindowRectEx>> = OnceLock::new();
static DIRECTDRAW_CREATE_HOOK: OnceLock<GenericDetour<FnDirectDrawCreate>> = OnceLock::new();
static DIRECTDRAW_CREATE_EX_HOOK: OnceLock<GenericDetour<FnDirectDrawCreateEx>> = OnceLock::new();
static DIRECT3D_CREATE9_HOOK: OnceLock<GenericDetour<FnDirect3DCreate9>> = OnceLock::new();
static DIRECT3D_CREATE9_EX_HOOK: OnceLock<GenericDetour<FnDirect3DCreate9Ex>> = OnceLock::new();

#[derive(Default)]
pub struct HookManager {
    installed: bool,
}

impl HookManager {
    pub fn install(&mut self, sender: Sender<Event>) -> Result<(), String> {
        if self.installed {
            return Ok(());
        }

        let sender_slot = EVENT_SENDER.get_or_init(|| Mutex::new(None));
        {
            let mut guard = sender_slot
                .lock()
                .map_err(|_| "event sender lock poisoned".to_owned())?;
            *guard = Some(sender);
        }

        let create_target: FnCreateWindowExW =
            unsafe { resolve_proc_in_module(b"user32.dll\0", b"CreateWindowExW\0")? };
        let set_window_pos_target: FnSetWindowPos =
            unsafe { resolve_proc_in_module(b"user32.dll\0", b"SetWindowPos\0")? };
        let move_window_target: FnMoveWindow =
            unsafe { resolve_proc_in_module(b"user32.dll\0", b"MoveWindow\0")? };
        let change_display_target: FnChangeDisplaySettingsExW =
            unsafe { resolve_proc_in_module(b"user32.dll\0", b"ChangeDisplaySettingsExW\0")? };
        let adjust_rect_target: FnAdjustWindowRectEx =
            unsafe { resolve_proc_in_module(b"user32.dll\0", b"AdjustWindowRectEx\0")? };
        let directdraw_create_target: Option<FnDirectDrawCreate> =
            unsafe { try_resolve_proc_in_module(b"ddraw.dll\0", b"DirectDrawCreate\0")? };
        let directdraw_create_ex_target: Option<FnDirectDrawCreateEx> =
            unsafe { try_resolve_proc_in_module(b"ddraw.dll\0", b"DirectDrawCreateEx\0")? };
        let direct3d_create9_target: Option<FnDirect3DCreate9> =
            unsafe { try_resolve_proc_in_module(b"d3d9.dll\0", b"Direct3DCreate9\0")? };
        let direct3d_create9_ex_target: Option<FnDirect3DCreate9Ex> =
            unsafe { try_resolve_proc_in_module(b"d3d9.dll\0", b"Direct3DCreate9Ex\0")? };

        let create_hook = unsafe { GenericDetour::new(create_target, create_window_exw_detour) }
            .map_err(|e| format!("CreateWindowExW init failed: {e}"))?;
        let set_window_pos_hook =
            unsafe { GenericDetour::new(set_window_pos_target, set_window_pos_detour) }
                .map_err(|e| format!("SetWindowPos init failed: {e}"))?;
        let move_window_hook =
            unsafe { GenericDetour::new(move_window_target, move_window_detour) }
                .map_err(|e| format!("MoveWindow init failed: {e}"))?;
        let change_display_hook = unsafe {
            GenericDetour::new(change_display_target, change_display_settings_exw_detour)
        }
        .map_err(|e| format!("ChangeDisplaySettingsExW init failed: {e}"))?;
        let adjust_rect_hook =
            unsafe { GenericDetour::new(adjust_rect_target, adjust_window_rect_ex_detour) }
                .map_err(|e| format!("AdjustWindowRectEx init failed: {e}"))?;
        let directdraw_create_hook = if let Some(target) = directdraw_create_target {
            Some(
                unsafe { GenericDetour::new(target, directdraw_create_detour) }
                    .map_err(|e| format!("DirectDrawCreate init failed: {e}"))?,
            )
        } else {
            None
        };
        let directdraw_create_ex_hook = if let Some(target) = directdraw_create_ex_target {
            Some(
                unsafe { GenericDetour::new(target, directdraw_create_ex_detour) }
                    .map_err(|e| format!("DirectDrawCreateEx init failed: {e}"))?,
            )
        } else {
            None
        };
        let direct3d_create9_hook = if let Some(target) = direct3d_create9_target {
            Some(
                unsafe { GenericDetour::new(target, direct3d_create9_detour) }
                    .map_err(|e| format!("Direct3DCreate9 init failed: {e}"))?,
            )
        } else {
            None
        };
        let direct3d_create9_ex_hook = if let Some(target) = direct3d_create9_ex_target {
            Some(
                unsafe { GenericDetour::new(target, direct3d_create9_ex_detour) }
                    .map_err(|e| format!("Direct3DCreate9Ex init failed: {e}"))?,
            )
        } else {
            None
        };

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
        if let Some(hook) = directdraw_create_hook {
            DIRECTDRAW_CREATE_HOOK
                .set(hook)
                .map_err(|_| "DirectDrawCreate hook was already set".to_owned())?;
        }
        if let Some(hook) = directdraw_create_ex_hook {
            DIRECTDRAW_CREATE_EX_HOOK
                .set(hook)
                .map_err(|_| "DirectDrawCreateEx hook was already set".to_owned())?;
        }
        if let Some(hook) = direct3d_create9_hook {
            DIRECT3D_CREATE9_HOOK
                .set(hook)
                .map_err(|_| "Direct3DCreate9 hook was already set".to_owned())?;
        }
        if let Some(hook) = direct3d_create9_ex_hook {
            DIRECT3D_CREATE9_EX_HOOK
                .set(hook)
                .map_err(|_| "Direct3DCreate9Ex hook was already set".to_owned())?;
        }

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
        if let Some(hook) = DIRECTDRAW_CREATE_HOOK.get() {
            unsafe { hook.enable() }.map_err(|e| format!("DirectDrawCreate enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECTDRAW_CREATE_EX_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("DirectDrawCreateEx enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECT3D_CREATE9_HOOK.get() {
            unsafe { hook.enable() }.map_err(|e| format!("Direct3DCreate9 enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECT3D_CREATE9_EX_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("Direct3DCreate9Ex enable failed: {e}"))?;
        }

        self.installed = true;
        Ok(())
    }
}

pub fn trigger_smoke_test_call() {
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

    dispatch_event(make_event(
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
    dispatch_event(make_event(
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
    dispatch_event(make_event(
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
    dispatch_event(make_event(
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
    dispatch_event(make_event(
        "AdjustWindowRectEx",
        format!("style=0x{style:08X} ex=0x{ex_style:08X} has_menu={has_menu}"),
        bool_result(result),
    ));
    result
}

unsafe extern "system" fn directdraw_create_detour(
    guid: *const c_void,
    direct_draw_out: *mut *mut c_void,
    unknown_outer: *mut c_void,
) -> i32 {
    let result = unsafe {
        DIRECTDRAW_CREATE_HOOK
            .get()
            .expect("DirectDrawCreate hook not installed")
            .call(guid, direct_draw_out, unknown_outer)
    };

    dispatch_event(make_event(
        "DirectDrawCreate",
        format!("guid_ptr={guid:p} out_ptr={direct_draw_out:p} outer_ptr={unknown_outer:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn directdraw_create_ex_detour(
    guid: *const c_void,
    direct_draw_out: *mut *mut c_void,
    iid: *const c_void,
    unknown_outer: *mut c_void,
) -> i32 {
    let result = unsafe {
        DIRECTDRAW_CREATE_EX_HOOK
            .get()
            .expect("DirectDrawCreateEx hook not installed")
            .call(guid, direct_draw_out, iid, unknown_outer)
    };

    dispatch_event(make_event(
        "DirectDrawCreateEx",
        format!(
            "guid_ptr={guid:p} out_ptr={direct_draw_out:p} iid_ptr={iid:p} outer_ptr={unknown_outer:p}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn direct3d_create9_detour(sdk_version: u32) -> *mut c_void {
    let result_ptr = unsafe {
        DIRECT3D_CREATE9_HOOK
            .get()
            .expect("Direct3DCreate9 hook not installed")
            .call(sdk_version)
    };

    dispatch_event(make_event(
        "Direct3DCreate9",
        format!("sdk_version={sdk_version}"),
        format!("PTR={result_ptr:p}"),
    ));
    result_ptr
}

unsafe extern "system" fn direct3d_create9_ex_detour(
    sdk_version: u32,
    direct3d_out: *mut *mut c_void,
) -> i32 {
    let result = unsafe {
        DIRECT3D_CREATE9_EX_HOOK
            .get()
            .expect("Direct3DCreate9Ex hook not installed")
            .call(sdk_version, direct3d_out)
    };

    dispatch_event(make_event(
        "Direct3DCreate9Ex",
        format!("sdk_version={sdk_version} out_ptr={direct3d_out:p}"),
        hresult_result(result),
    ));
    result
}

fn dispatch_event(event: Event) {
    if let Some(slot) = EVENT_SENDER.get() {
        if let Ok(guard) = slot.lock() {
            if let Some(sender) = guard.as_ref() {
                let _ = sender.send(event);
            }
        }
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

fn hresult_result(value: i32) -> String {
    format!("HRESULT=0x{:08X}", value as u32)
}

fn elapsed_ms() -> u64 {
    let started_at = START_TIME.get_or_init(Instant::now);
    let millis = started_at.elapsed().as_millis();
    millis.min(u64::MAX as u128) as u64
}

unsafe fn resolve_proc_in_module<T>(
    module_name: &'static [u8],
    proc_name: &'static [u8],
) -> Result<T, String> {
    let module = unsafe { LoadLibraryA(module_name.as_ptr()) };
    if module.is_null() {
        return Err(format!(
            "LoadLibraryA({}) failed",
            display_proc(module_name)
        ));
    }

    let proc = unsafe { GetProcAddress(module, proc_name.as_ptr()) };
    let proc =
        proc.ok_or_else(|| format!("GetProcAddress failed for {}", display_proc(proc_name)))?;

    Ok(proc_to_fn(proc))
}

unsafe fn try_resolve_proc_in_module<T>(
    module_name: &'static [u8],
    proc_name: &'static [u8],
) -> Result<Option<T>, String> {
    let module = unsafe { LoadLibraryA(module_name.as_ptr()) };
    if module.is_null() {
        return Ok(None);
    }

    let proc = unsafe { GetProcAddress(module, proc_name.as_ptr()) };
    Ok(proc.map(proc_to_fn))
}

fn proc_to_fn<T>(proc: unsafe extern "system" fn() -> isize) -> T {
    let raw = proc as *const ();
    unsafe { std::mem::transmute_copy(&raw) }
}

fn display_proc(name: &[u8]) -> String {
    let end = name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name.len());
    String::from_utf8_lossy(&name[..end]).to_string()
}
