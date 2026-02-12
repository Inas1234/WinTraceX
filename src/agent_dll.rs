mod model {
    pub mod event;
    pub mod ipc;
}

use model::event::Event;
use model::ipc::TRACE_UDP_BIND_ADDR;
use retour::GenericDetour;
use std::collections::HashSet;
use std::ffi::c_void;
use std::net::UdpSocket;
#[cfg(target_pointer_width = "32")]
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use windows_sys::Win32::Foundation::{CloseHandle, HINSTANCE, INVALID_HANDLE_VALUE, RECT};
use windows_sys::Win32::Graphics::Gdi::DEVMODEW;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32,
};
use windows_sys::Win32::System::LibraryLoader::{
    DisableThreadLibraryCalls, GetModuleFileNameW, GetModuleHandleA, GetProcAddress, LoadLibraryA,
};
use windows_sys::Win32::System::Memory::{
    MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
    PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_NOACCESS, VirtualQuery,
};
#[cfg(target_pointer_width = "32")]
use windows_sys::Win32::System::Memory::{MEM_RESERVE, PAGE_READWRITE, VirtualAlloc};
use windows_sys::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
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
type FnDirectDrawCreate =
    unsafe extern "system" fn(*const c_void, *mut *mut c_void, *mut c_void) -> i32;
type FnDirectDrawCreateEx =
    unsafe extern "system" fn(*const c_void, *mut *mut c_void, *const c_void, *mut c_void) -> i32;
type FnDirectDrawCreateClipper =
    unsafe extern "system" fn(u32, *mut *mut c_void, *mut c_void) -> i32;
type FnDirectDrawEnumerateA = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type FnDirectDrawEnumerateW = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type FnDirectDrawEnumerateExA = unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> i32;
type FnDirectDrawEnumerateExW = unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> i32;
type FnDdCreateSurface =
    unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void, *mut c_void) -> i32;
type FnDdSetCooperativeLevel = unsafe extern "system" fn(*mut c_void, isize, u32) -> i32;
type FnDdSetDisplayMode = unsafe extern "system" fn(*mut c_void, u32, u32, u32) -> i32;
type FnDdSetDisplayModeEx = unsafe extern "system" fn(*mut c_void, u32, u32, u32, u32, u32) -> i32;
type FnDdRestoreDisplayMode = unsafe extern "system" fn(*mut c_void) -> i32;
type FnDdWaitForVerticalBlank = unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> i32;
type FnDdQueryInterface =
    unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> i32;
type FnDdRelease = unsafe extern "system" fn(*mut c_void) -> u32;
type FnDdSurfaceBlt = unsafe extern "system" fn(
    *mut c_void,
    *mut RECT,
    *mut c_void,
    *mut RECT,
    u32,
    *mut c_void,
) -> i32;
type FnDdSurfaceBltFast =
    unsafe extern "system" fn(*mut c_void, u32, u32, *mut c_void, *mut RECT, u32) -> i32;
type FnDdSurfaceFlip = unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> i32;
type FnDdSurfaceLock =
    unsafe extern "system" fn(*mut c_void, *mut RECT, *mut c_void, u32, *mut c_void) -> i32;
type FnDdSurfaceUnlock = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type FnDdSurfaceGetDC = unsafe extern "system" fn(*mut c_void, *mut isize) -> i32;
type FnDdSurfaceReleaseDC = unsafe extern "system" fn(*mut c_void, isize) -> i32;
type FnDdSurfaceIsLost = unsafe extern "system" fn(*mut c_void) -> i32;
type FnDdSurfaceRestore = unsafe extern "system" fn(*mut c_void) -> i32;
type FnDdSurfaceGetSurfaceDesc = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type FnDdSurfaceGetAttachedSurface =
    unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32;
type FnDdSurfaceSetClipper = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type FnDdSurfaceSetPalette = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type FnDirect3DCreate9 = unsafe extern "system" fn(u32) -> *mut c_void;
type FnDirect3DCreate9Ex = unsafe extern "system" fn(u32, *mut *mut c_void) -> i32;
type FnLoadLibraryA = unsafe extern "system" fn(*const u8) -> *mut c_void;
type FnLoadLibraryW = unsafe extern "system" fn(*const u16) -> *mut c_void;
type FnLoadLibraryExA = unsafe extern "system" fn(*const u8, *mut c_void, u32) -> *mut c_void;
type FnLoadLibraryExW = unsafe extern "system" fn(*const u16, *mut c_void, u32) -> *mut c_void;
type FnCoCreateInstance = unsafe extern "system" fn(
    *const c_void,
    *mut c_void,
    u32,
    *const c_void,
    *mut *mut c_void,
) -> i32;
#[repr(C)]
struct MultiQi {
    riid: *const c_void,
    out_object: *mut c_void,
    hr: i32,
}
type FnCoCreateInstanceEx = unsafe extern "system" fn(
    *const c_void,
    *mut c_void,
    u32,
    *mut c_void,
    u32,
    *mut MultiQi,
) -> i32;
type FnCreateDXGIFactory = unsafe extern "system" fn(*const c_void, *mut *mut c_void) -> i32;
type FnCreateDXGIFactory1 = unsafe extern "system" fn(*const c_void, *mut *mut c_void) -> i32;
type FnD3D11CreateDevice = unsafe extern "system" fn(
    *mut c_void,
    u32,
    *mut c_void,
    u32,
    *const u32,
    u32,
    u32,
    *mut *mut c_void,
    *mut u32,
    *mut *mut c_void,
) -> i32;
type FnD3D11CreateDeviceAndSwapChain = unsafe extern "system" fn(
    *mut c_void,
    u32,
    *mut c_void,
    u32,
    *const u32,
    u32,
    u32,
    *const c_void,
    *mut *mut c_void,
    *mut *mut c_void,
    *mut u32,
    *mut *mut c_void,
) -> i32;

static START_TIME: OnceLock<Instant> = OnceLock::new();
static UDP_SOCKET: OnceLock<Option<UdpSocket>> = OnceLock::new();
static DLL_LOAD_KEYS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
#[cfg(target_pointer_width = "32")]
static DIRECTDRAW_VTABLE_USAGE_REPORTED: AtomicU8 = AtomicU8::new(0);
#[cfg(target_pointer_width = "32")]
static DIRECTDRAW_PATCHED_INTERFACES: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();
#[cfg(target_pointer_width = "32")]
static DIRECTDRAWSURFACE_PATCHED_INTERFACES: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();

static CREATE_WINDOW_EXW_HOOK: OnceLock<GenericDetour<FnCreateWindowExW>> = OnceLock::new();
static SET_WINDOW_POS_HOOK: OnceLock<GenericDetour<FnSetWindowPos>> = OnceLock::new();
static MOVE_WINDOW_HOOK: OnceLock<GenericDetour<FnMoveWindow>> = OnceLock::new();
static CHANGE_DISPLAY_SETTINGS_EXW_HOOK: OnceLock<GenericDetour<FnChangeDisplaySettingsExW>> =
    OnceLock::new();
static ADJUST_WINDOW_RECT_EX_HOOK: OnceLock<GenericDetour<FnAdjustWindowRectEx>> = OnceLock::new();
static DIRECTDRAW_CREATE_HOOK: OnceLock<GenericDetour<FnDirectDrawCreate>> = OnceLock::new();
static DIRECTDRAW_CREATE_EX_HOOK: OnceLock<GenericDetour<FnDirectDrawCreateEx>> = OnceLock::new();
static DIRECTDRAW_CREATE_CLIPPER_HOOK: OnceLock<GenericDetour<FnDirectDrawCreateClipper>> =
    OnceLock::new();
static DIRECTDRAW_ENUMERATE_A_HOOK: OnceLock<GenericDetour<FnDirectDrawEnumerateA>> =
    OnceLock::new();
static DIRECTDRAW_ENUMERATE_W_HOOK: OnceLock<GenericDetour<FnDirectDrawEnumerateW>> =
    OnceLock::new();
static DIRECTDRAW_ENUMERATE_EX_A_HOOK: OnceLock<GenericDetour<FnDirectDrawEnumerateExA>> =
    OnceLock::new();
static DIRECTDRAW_ENUMERATE_EX_W_HOOK: OnceLock<GenericDetour<FnDirectDrawEnumerateExW>> =
    OnceLock::new();
static DD_CREATE_SURFACE_HOOK: OnceLock<GenericDetour<FnDdCreateSurface>> = OnceLock::new();
static DD_CREATE_SURFACE_ALT_HOOK: OnceLock<GenericDetour<FnDdCreateSurface>> = OnceLock::new();
static DD_CREATE_SURFACE_TARGET: OnceLock<usize> = OnceLock::new();
static DD_CREATE_SURFACE_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SET_COOPERATIVE_LEVEL_HOOK: OnceLock<GenericDetour<FnDdSetCooperativeLevel>> =
    OnceLock::new();
static DD_SET_COOPERATIVE_LEVEL_ALT_HOOK: OnceLock<GenericDetour<FnDdSetCooperativeLevel>> =
    OnceLock::new();
static DD_SET_COOPERATIVE_LEVEL_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SET_COOPERATIVE_LEVEL_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SET_DISPLAY_MODE_HOOK: OnceLock<GenericDetour<FnDdSetDisplayMode>> = OnceLock::new();
static DD_SET_DISPLAY_MODE_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SET_DISPLAY_MODE_ALT_HOOK: OnceLock<GenericDetour<FnDdSetDisplayMode>> = OnceLock::new();
static DD_SET_DISPLAY_MODE_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SET_DISPLAY_MODE_EX_HOOK: OnceLock<GenericDetour<FnDdSetDisplayModeEx>> = OnceLock::new();
static DD_SET_DISPLAY_MODE_EX_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SET_DISPLAY_MODE_EX_ALT_HOOK: OnceLock<GenericDetour<FnDdSetDisplayModeEx>> =
    OnceLock::new();
static DD_SET_DISPLAY_MODE_EX_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_RESTORE_DISPLAY_MODE_HOOK: OnceLock<GenericDetour<FnDdRestoreDisplayMode>> =
    OnceLock::new();
static DD_RESTORE_DISPLAY_MODE_TARGET: OnceLock<usize> = OnceLock::new();
static DD_RESTORE_DISPLAY_MODE_ALT_HOOK: OnceLock<GenericDetour<FnDdRestoreDisplayMode>> =
    OnceLock::new();
static DD_RESTORE_DISPLAY_MODE_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_WAIT_FOR_VBLANK_HOOK: OnceLock<GenericDetour<FnDdWaitForVerticalBlank>> = OnceLock::new();
static DD_WAIT_FOR_VBLANK_TARGET: OnceLock<usize> = OnceLock::new();
static DD_WAIT_FOR_VBLANK_ALT_HOOK: OnceLock<GenericDetour<FnDdWaitForVerticalBlank>> =
    OnceLock::new();
static DD_WAIT_FOR_VBLANK_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_QUERY_INTERFACE_HOOK: OnceLock<GenericDetour<FnDdQueryInterface>> = OnceLock::new();
static DD_SURFACE_BLT_HOOK: OnceLock<GenericDetour<FnDdSurfaceBlt>> = OnceLock::new();
static DD_SURFACE_BLT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_BLT_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceBlt>> = OnceLock::new();
static DD_SURFACE_BLT_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_BLTFAST_HOOK: OnceLock<GenericDetour<FnDdSurfaceBltFast>> = OnceLock::new();
static DD_SURFACE_BLTFAST_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_BLTFAST_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceBltFast>> = OnceLock::new();
static DD_SURFACE_BLTFAST_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_FLIP_HOOK: OnceLock<GenericDetour<FnDdSurfaceFlip>> = OnceLock::new();
static DD_SURFACE_FLIP_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_FLIP_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceFlip>> = OnceLock::new();
static DD_SURFACE_FLIP_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_LOCK_HOOK: OnceLock<GenericDetour<FnDdSurfaceLock>> = OnceLock::new();
static DD_SURFACE_LOCK_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_LOCK_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceLock>> = OnceLock::new();
static DD_SURFACE_LOCK_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_UNLOCK_HOOK: OnceLock<GenericDetour<FnDdSurfaceUnlock>> = OnceLock::new();
static DD_SURFACE_UNLOCK_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_UNLOCK_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceUnlock>> = OnceLock::new();
static DD_SURFACE_UNLOCK_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_GETDC_HOOK: OnceLock<GenericDetour<FnDdSurfaceGetDC>> = OnceLock::new();
static DD_SURFACE_GETDC_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_GETDC_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceGetDC>> = OnceLock::new();
static DD_SURFACE_GETDC_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_RELEASEDC_HOOK: OnceLock<GenericDetour<FnDdSurfaceReleaseDC>> = OnceLock::new();
static DD_SURFACE_RELEASEDC_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_RELEASEDC_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceReleaseDC>> =
    OnceLock::new();
static DD_SURFACE_RELEASEDC_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_ISLOST_HOOK: OnceLock<GenericDetour<FnDdSurfaceIsLost>> = OnceLock::new();
static DD_SURFACE_ISLOST_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_ISLOST_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceIsLost>> = OnceLock::new();
static DD_SURFACE_ISLOST_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_RESTORE_HOOK: OnceLock<GenericDetour<FnDdSurfaceRestore>> = OnceLock::new();
static DD_SURFACE_RESTORE_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_RESTORE_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceRestore>> = OnceLock::new();
static DD_SURFACE_RESTORE_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_GETDESC_HOOK: OnceLock<GenericDetour<FnDdSurfaceGetSurfaceDesc>> =
    OnceLock::new();
static DD_SURFACE_GETDESC_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_GETDESC_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceGetSurfaceDesc>> =
    OnceLock::new();
static DD_SURFACE_GETDESC_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_GETATTACHED_HOOK: OnceLock<GenericDetour<FnDdSurfaceGetAttachedSurface>> =
    OnceLock::new();
static DD_SURFACE_GETATTACHED_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_GETATTACHED_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceGetAttachedSurface>> =
    OnceLock::new();
static DD_SURFACE_GETATTACHED_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_SETCLIPPER_HOOK: OnceLock<GenericDetour<FnDdSurfaceSetClipper>> = OnceLock::new();
static DD_SURFACE_SETCLIPPER_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_SETCLIPPER_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceSetClipper>> =
    OnceLock::new();
static DD_SURFACE_SETCLIPPER_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_SETPALETTE_HOOK: OnceLock<GenericDetour<FnDdSurfaceSetPalette>> = OnceLock::new();
static DD_SURFACE_SETPALETTE_TARGET: OnceLock<usize> = OnceLock::new();
static DD_SURFACE_SETPALETTE_ALT_HOOK: OnceLock<GenericDetour<FnDdSurfaceSetPalette>> =
    OnceLock::new();
static DD_SURFACE_SETPALETTE_ALT_TARGET: OnceLock<usize> = OnceLock::new();
static DIRECT3D_CREATE9_HOOK: OnceLock<GenericDetour<FnDirect3DCreate9>> = OnceLock::new();
static DIRECT3D_CREATE9_EX_HOOK: OnceLock<GenericDetour<FnDirect3DCreate9Ex>> = OnceLock::new();
static LOAD_LIBRARY_A_HOOK: OnceLock<GenericDetour<FnLoadLibraryA>> = OnceLock::new();
static LOAD_LIBRARY_W_HOOK: OnceLock<GenericDetour<FnLoadLibraryW>> = OnceLock::new();
static LOAD_LIBRARY_EX_A_HOOK: OnceLock<GenericDetour<FnLoadLibraryExA>> = OnceLock::new();
static LOAD_LIBRARY_EX_W_HOOK: OnceLock<GenericDetour<FnLoadLibraryExW>> = OnceLock::new();
static CO_CREATE_INSTANCE_HOOK: OnceLock<GenericDetour<FnCoCreateInstance>> = OnceLock::new();
static CO_CREATE_INSTANCE_EX_HOOK: OnceLock<GenericDetour<FnCoCreateInstanceEx>> = OnceLock::new();
static CREATE_DXGI_FACTORY_HOOK: OnceLock<GenericDetour<FnCreateDXGIFactory>> = OnceLock::new();
static CREATE_DXGI_FACTORY1_HOOK: OnceLock<GenericDetour<FnCreateDXGIFactory1>> = OnceLock::new();
static D3D11_CREATE_DEVICE_HOOK: OnceLock<GenericDetour<FnD3D11CreateDevice>> = OnceLock::new();
static D3D11_CREATE_DEVICE_AND_SWAP_CHAIN_HOOK: OnceLock<
    GenericDetour<FnD3D11CreateDeviceAndSwapChain>,
> = OnceLock::new();
static OPTIONAL_HOOK_INSTALL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const CLSID_DIRECTDRAW: Guid = Guid {
    data1: 0xD7B70EE0,
    data2: 0x4340,
    data3: 0x11CF,
    data4: [0xB0, 0x63, 0x00, 0x20, 0xAF, 0xC2, 0xCD, 0x35],
};
const CLSID_DIRECTDRAW7: Guid = Guid {
    data1: 0x3C305196,
    data2: 0x50DB,
    data3: 0x11D3,
    data4: [0x9C, 0xFE, 0x00, 0xC0, 0x4F, 0xD9, 0x30, 0xC5],
};
const IID_IDIRECTDRAW: Guid = Guid {
    data1: 0x6C14DB80,
    data2: 0xA733,
    data3: 0x11CE,
    data4: [0xA5, 0x21, 0x00, 0x20, 0xAF, 0x0B, 0xE5, 0x60],
};
const IID_IDIRECTDRAW2: Guid = Guid {
    data1: 0xB3A6F3E0,
    data2: 0x2B43,
    data3: 0x11CF,
    data4: [0xA2, 0xDE, 0x00, 0xAA, 0x00, 0xB9, 0x33, 0x56],
};
const IID_IDIRECTDRAW4: Guid = Guid {
    data1: 0x9C59509A,
    data2: 0x39BD,
    data3: 0x11D1,
    data4: [0x8C, 0x4A, 0x00, 0xC0, 0x4F, 0xD9, 0x30, 0xC5],
};
const IID_IDIRECTDRAW7: Guid = Guid {
    data1: 0x15E65EC0,
    data2: 0x3B9C,
    data3: 0x11D2,
    data4: [0xB9, 0x2F, 0x00, 0x60, 0x97, 0x97, 0xEA, 0x5B],
};
const AGENT_BUILD_TAG: &str = "ddraw-hooks-2026-02-12-r5";
// IDirectDraw(7) vtable layout:
// 0 QI, 1 AddRef, 2 Release, ..., 20 SetCooperativeLevel, 21 SetDisplayMode, 22 WaitForVerticalBlank
const DD_METHOD_RESTORE_DISPLAY_MODE_INDEX: usize = 19;
const DD_METHOD_SET_COOPERATIVE_LEVEL_INDEX: usize = 20;
const DD_METHOD_SET_DISPLAY_MODE_INDEX: usize = 21;
const DD_METHOD_WAIT_FOR_VERTICAL_BLANK_INDEX: usize = 22;
const DDS_METHOD_BLT_INDEX: usize = 5;
const DDS_METHOD_BLTFAST_INDEX: usize = 7;
const DDS_METHOD_FLIP_INDEX: usize = 11;
const DDS_METHOD_GETATTACHED_INDEX: usize = 12;
const DDS_METHOD_GETDESC_INDEX: usize = 22;
const DDS_METHOD_ISLOST_INDEX: usize = 24;
const DDS_METHOD_GETDC_INDEX: usize = 17;
const DDS_METHOD_LOCK_INDEX: usize = 25;
const DDS_METHOD_RELEASEDC_INDEX: usize = 26;
const DDS_METHOD_RESTORE_INDEX: usize = 27;
const DDS_METHOD_SETCLIPPER_INDEX: usize = 28;
const DDS_METHOD_SETPALETTE_INDEX: usize = 31;
const DDS_METHOD_UNLOCK_INDEX: usize = 32;

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(
    module: HINSTANCE,
    reason: u32,
    _reserved: *mut c_void,
) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        unsafe {
            DisableThreadLibraryCalls(module);
        }
    }

    1
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn InitializeAgent(_param: *mut c_void) -> u32 {
    if install_hooks().is_err() {
        return 0;
    }

    emit_agent_build();
    emit_loaded_modules_snapshot();
    try_probe_install_directdraw_vtable_hooks();
    emit_directdraw_hook_status();
    trigger_smoke_test_call();
    1
}

fn install_hooks() -> Result<(), String> {
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
    let load_library_a_target: FnLoadLibraryA =
        unsafe { resolve_proc_in_module(b"kernel32.dll\0", b"LoadLibraryA\0")? };
    let load_library_w_target: FnLoadLibraryW =
        unsafe { resolve_proc_in_module(b"kernel32.dll\0", b"LoadLibraryW\0")? };
    let load_library_ex_a_target: FnLoadLibraryExA =
        unsafe { resolve_proc_in_module(b"kernel32.dll\0", b"LoadLibraryExA\0")? };
    let load_library_ex_w_target: FnLoadLibraryExW =
        unsafe { resolve_proc_in_module(b"kernel32.dll\0", b"LoadLibraryExW\0")? };
    let co_create_instance_target: Option<FnCoCreateInstance> =
        unsafe { try_resolve_proc_in_module(b"ole32.dll\0", b"CoCreateInstance\0") };
    let co_create_instance_ex_target: Option<FnCoCreateInstanceEx> =
        unsafe { try_resolve_proc_in_module(b"ole32.dll\0", b"CoCreateInstanceEx\0") };
    let directdraw_create_target: Option<FnDirectDrawCreate> =
        unsafe { try_resolve_proc_in_module(b"ddraw.dll\0", b"DirectDrawCreate\0") };
    let directdraw_create_ex_target: Option<FnDirectDrawCreateEx> =
        unsafe { try_resolve_proc_in_module(b"ddraw.dll\0", b"DirectDrawCreateEx\0") };
    let directdraw_create_clipper_target: Option<FnDirectDrawCreateClipper> =
        unsafe { try_resolve_proc_in_module(b"ddraw.dll\0", b"DirectDrawCreateClipper\0") };
    let directdraw_enumerate_a_target: Option<FnDirectDrawEnumerateA> =
        unsafe { try_resolve_proc_in_module(b"ddraw.dll\0", b"DirectDrawEnumerateA\0") };
    let directdraw_enumerate_w_target: Option<FnDirectDrawEnumerateW> =
        unsafe { try_resolve_proc_in_module(b"ddraw.dll\0", b"DirectDrawEnumerateW\0") };
    let directdraw_enumerate_ex_a_target: Option<FnDirectDrawEnumerateExA> =
        unsafe { try_resolve_proc_in_module(b"ddraw.dll\0", b"DirectDrawEnumerateExA\0") };
    let directdraw_enumerate_ex_w_target: Option<FnDirectDrawEnumerateExW> =
        unsafe { try_resolve_proc_in_module(b"ddraw.dll\0", b"DirectDrawEnumerateExW\0") };
    let direct3d_create9_target: Option<FnDirect3DCreate9> =
        unsafe { try_resolve_proc_in_loaded_module(b"d3d9.dll\0", b"Direct3DCreate9\0") };
    let direct3d_create9_ex_target: Option<FnDirect3DCreate9Ex> =
        unsafe { try_resolve_proc_in_loaded_module(b"d3d9.dll\0", b"Direct3DCreate9Ex\0") };
    let create_dxgi_factory_target: Option<FnCreateDXGIFactory> =
        unsafe { try_resolve_proc_in_loaded_module(b"dxgi.dll\0", b"CreateDXGIFactory\0") };
    let create_dxgi_factory1_target: Option<FnCreateDXGIFactory1> =
        unsafe { try_resolve_proc_in_loaded_module(b"dxgi.dll\0", b"CreateDXGIFactory1\0") };
    let d3d11_create_device_target: Option<FnD3D11CreateDevice> =
        unsafe { try_resolve_proc_in_loaded_module(b"d3d11.dll\0", b"D3D11CreateDevice\0") };
    let d3d11_create_device_and_swap_chain_target: Option<FnD3D11CreateDeviceAndSwapChain> = unsafe {
        try_resolve_proc_in_loaded_module(b"d3d11.dll\0", b"D3D11CreateDeviceAndSwapChain\0")
    };

    let create_hook = unsafe { GenericDetour::new(create_target, create_window_exw_detour) }
        .map_err(|e| format!("CreateWindowExW init failed: {e}"))?;
    let set_window_pos_hook =
        unsafe { GenericDetour::new(set_window_pos_target, set_window_pos_detour) }
            .map_err(|e| format!("SetWindowPos init failed: {e}"))?;
    let move_window_hook = unsafe { GenericDetour::new(move_window_target, move_window_detour) }
        .map_err(|e| format!("MoveWindow init failed: {e}"))?;
    let change_display_hook =
        unsafe { GenericDetour::new(change_display_target, change_display_settings_exw_detour) }
            .map_err(|e| format!("ChangeDisplaySettingsExW init failed: {e}"))?;
    let adjust_rect_hook =
        unsafe { GenericDetour::new(adjust_rect_target, adjust_window_rect_ex_detour) }
            .map_err(|e| format!("AdjustWindowRectEx init failed: {e}"))?;
    let load_library_a_hook =
        unsafe { GenericDetour::new(load_library_a_target, load_library_a_detour) }
            .map_err(|e| format!("LoadLibraryA init failed: {e}"))?;
    let load_library_w_hook =
        unsafe { GenericDetour::new(load_library_w_target, load_library_w_detour) }
            .map_err(|e| format!("LoadLibraryW init failed: {e}"))?;
    let load_library_ex_a_hook =
        unsafe { GenericDetour::new(load_library_ex_a_target, load_library_ex_a_detour) }
            .map_err(|e| format!("LoadLibraryExA init failed: {e}"))?;
    let load_library_ex_w_hook =
        unsafe { GenericDetour::new(load_library_ex_w_target, load_library_ex_w_detour) }
            .map_err(|e| format!("LoadLibraryExW init failed: {e}"))?;
    let co_create_instance_hook = if let Some(target) = co_create_instance_target {
        Some(
            unsafe { GenericDetour::new(target, co_create_instance_detour) }
                .map_err(|e| format!("CoCreateInstance init failed: {e}"))?,
        )
    } else {
        None
    };
    let co_create_instance_ex_hook = if let Some(target) = co_create_instance_ex_target {
        Some(
            unsafe { GenericDetour::new(target, co_create_instance_ex_detour) }
                .map_err(|e| format!("CoCreateInstanceEx init failed: {e}"))?,
        )
    } else {
        None
    };
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
    let directdraw_create_clipper_hook = if let Some(target) = directdraw_create_clipper_target {
        Some(
            unsafe { GenericDetour::new(target, directdraw_create_clipper_detour) }
                .map_err(|e| format!("DirectDrawCreateClipper init failed: {e}"))?,
        )
    } else {
        None
    };
    let directdraw_enumerate_a_hook = if let Some(target) = directdraw_enumerate_a_target {
        Some(
            unsafe { GenericDetour::new(target, directdraw_enumerate_a_detour) }
                .map_err(|e| format!("DirectDrawEnumerateA init failed: {e}"))?,
        )
    } else {
        None
    };
    let directdraw_enumerate_w_hook = if let Some(target) = directdraw_enumerate_w_target {
        Some(
            unsafe { GenericDetour::new(target, directdraw_enumerate_w_detour) }
                .map_err(|e| format!("DirectDrawEnumerateW init failed: {e}"))?,
        )
    } else {
        None
    };
    let directdraw_enumerate_ex_a_hook = if let Some(target) = directdraw_enumerate_ex_a_target {
        Some(
            unsafe { GenericDetour::new(target, directdraw_enumerate_ex_a_detour) }
                .map_err(|e| format!("DirectDrawEnumerateExA init failed: {e}"))?,
        )
    } else {
        None
    };
    let directdraw_enumerate_ex_w_hook = if let Some(target) = directdraw_enumerate_ex_w_target {
        Some(
            unsafe { GenericDetour::new(target, directdraw_enumerate_ex_w_detour) }
                .map_err(|e| format!("DirectDrawEnumerateExW init failed: {e}"))?,
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
    let create_dxgi_factory_hook = if let Some(target) = create_dxgi_factory_target {
        Some(
            unsafe { GenericDetour::new(target, create_dxgi_factory_detour) }
                .map_err(|e| format!("CreateDXGIFactory init failed: {e}"))?,
        )
    } else {
        None
    };
    let create_dxgi_factory1_hook = if let Some(target) = create_dxgi_factory1_target {
        Some(
            unsafe { GenericDetour::new(target, create_dxgi_factory1_detour) }
                .map_err(|e| format!("CreateDXGIFactory1 init failed: {e}"))?,
        )
    } else {
        None
    };
    let d3d11_create_device_hook = if let Some(target) = d3d11_create_device_target {
        Some(
            unsafe { GenericDetour::new(target, d3d11_create_device_detour) }
                .map_err(|e| format!("D3D11CreateDevice init failed: {e}"))?,
        )
    } else {
        None
    };
    let d3d11_create_device_and_swap_chain_hook =
        if let Some(target) = d3d11_create_device_and_swap_chain_target {
            Some(
                unsafe { GenericDetour::new(target, d3d11_create_device_and_swap_chain_detour) }
                    .map_err(|e| format!("D3D11CreateDeviceAndSwapChain init failed: {e}"))?,
            )
        } else {
            None
        };

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
        LOAD_LIBRARY_A_HOOK
            .set(load_library_a_hook)
            .map_err(|_| "LoadLibraryA hook was already set".to_owned())?;
        LOAD_LIBRARY_W_HOOK
            .set(load_library_w_hook)
            .map_err(|_| "LoadLibraryW hook was already set".to_owned())?;
        LOAD_LIBRARY_EX_A_HOOK
            .set(load_library_ex_a_hook)
            .map_err(|_| "LoadLibraryExA hook was already set".to_owned())?;
        LOAD_LIBRARY_EX_W_HOOK
            .set(load_library_ex_w_hook)
            .map_err(|_| "LoadLibraryExW hook was already set".to_owned())?;
        if let Some(hook) = co_create_instance_hook {
            CO_CREATE_INSTANCE_HOOK
                .set(hook)
                .map_err(|_| "CoCreateInstance hook was already set".to_owned())?;
        }
        if let Some(hook) = co_create_instance_ex_hook {
            CO_CREATE_INSTANCE_EX_HOOK
                .set(hook)
                .map_err(|_| "CoCreateInstanceEx hook was already set".to_owned())?;
        }
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
        if let Some(hook) = directdraw_create_clipper_hook {
            DIRECTDRAW_CREATE_CLIPPER_HOOK
                .set(hook)
                .map_err(|_| "DirectDrawCreateClipper hook was already set".to_owned())?;
        }
        if let Some(hook) = directdraw_enumerate_a_hook {
            DIRECTDRAW_ENUMERATE_A_HOOK
                .set(hook)
                .map_err(|_| "DirectDrawEnumerateA hook was already set".to_owned())?;
        }
        if let Some(hook) = directdraw_enumerate_w_hook {
            DIRECTDRAW_ENUMERATE_W_HOOK
                .set(hook)
                .map_err(|_| "DirectDrawEnumerateW hook was already set".to_owned())?;
        }
        if let Some(hook) = directdraw_enumerate_ex_a_hook {
            DIRECTDRAW_ENUMERATE_EX_A_HOOK
                .set(hook)
                .map_err(|_| "DirectDrawEnumerateExA hook was already set".to_owned())?;
        }
        if let Some(hook) = directdraw_enumerate_ex_w_hook {
            DIRECTDRAW_ENUMERATE_EX_W_HOOK
                .set(hook)
                .map_err(|_| "DirectDrawEnumerateExW hook was already set".to_owned())?;
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
        if let Some(hook) = create_dxgi_factory_hook {
            CREATE_DXGI_FACTORY_HOOK
                .set(hook)
                .map_err(|_| "CreateDXGIFactory hook was already set".to_owned())?;
        }
        if let Some(hook) = create_dxgi_factory1_hook {
            CREATE_DXGI_FACTORY1_HOOK
                .set(hook)
                .map_err(|_| "CreateDXGIFactory1 hook was already set".to_owned())?;
        }
        if let Some(hook) = d3d11_create_device_hook {
            D3D11_CREATE_DEVICE_HOOK
                .set(hook)
                .map_err(|_| "D3D11CreateDevice hook was already set".to_owned())?;
        }
        if let Some(hook) = d3d11_create_device_and_swap_chain_hook {
            D3D11_CREATE_DEVICE_AND_SWAP_CHAIN_HOOK
                .set(hook)
                .map_err(|_| "D3D11CreateDeviceAndSwapChain hook was already set".to_owned())?;
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
        unsafe {
            LOAD_LIBRARY_A_HOOK
                .get()
                .expect("LoadLibraryA hook missing after set")
                .enable()
        }
        .map_err(|e| format!("LoadLibraryA enable failed: {e}"))?;
        unsafe {
            LOAD_LIBRARY_W_HOOK
                .get()
                .expect("LoadLibraryW hook missing after set")
                .enable()
        }
        .map_err(|e| format!("LoadLibraryW enable failed: {e}"))?;
        unsafe {
            LOAD_LIBRARY_EX_A_HOOK
                .get()
                .expect("LoadLibraryExA hook missing after set")
                .enable()
        }
        .map_err(|e| format!("LoadLibraryExA enable failed: {e}"))?;
        unsafe {
            LOAD_LIBRARY_EX_W_HOOK
                .get()
                .expect("LoadLibraryExW hook missing after set")
                .enable()
        }
        .map_err(|e| format!("LoadLibraryExW enable failed: {e}"))?;
        if let Some(hook) = CO_CREATE_INSTANCE_HOOK.get() {
            unsafe { hook.enable() }.map_err(|e| format!("CoCreateInstance enable failed: {e}"))?;
        }
        if let Some(hook) = CO_CREATE_INSTANCE_EX_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("CoCreateInstanceEx enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECTDRAW_CREATE_HOOK.get() {
            unsafe { hook.enable() }.map_err(|e| format!("DirectDrawCreate enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECTDRAW_CREATE_EX_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("DirectDrawCreateEx enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECTDRAW_CREATE_CLIPPER_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("DirectDrawCreateClipper enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECTDRAW_ENUMERATE_A_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("DirectDrawEnumerateA enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECTDRAW_ENUMERATE_W_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("DirectDrawEnumerateW enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECTDRAW_ENUMERATE_EX_A_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("DirectDrawEnumerateExA enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECTDRAW_ENUMERATE_EX_W_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("DirectDrawEnumerateExW enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECT3D_CREATE9_HOOK.get() {
            unsafe { hook.enable() }.map_err(|e| format!("Direct3DCreate9 enable failed: {e}"))?;
        }
        if let Some(hook) = DIRECT3D_CREATE9_EX_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("Direct3DCreate9Ex enable failed: {e}"))?;
        }
        if let Some(hook) = CREATE_DXGI_FACTORY_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("CreateDXGIFactory enable failed: {e}"))?;
        }
        if let Some(hook) = CREATE_DXGI_FACTORY1_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("CreateDXGIFactory1 enable failed: {e}"))?;
        }
        if let Some(hook) = D3D11_CREATE_DEVICE_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("D3D11CreateDevice enable failed: {e}"))?;
        }
        if let Some(hook) = D3D11_CREATE_DEVICE_AND_SWAP_CHAIN_HOOK.get() {
            unsafe { hook.enable() }
                .map_err(|e| format!("D3D11CreateDeviceAndSwapChain enable failed: {e}"))?;
        }
        let _ = try_install_optional_graphics_hooks();
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
    if hresult_succeeded(result) && !direct_draw_out.is_null() {
        let direct_draw = unsafe { *direct_draw_out };
        try_install_directdraw_object_hooks(direct_draw, "DirectDrawCreate");
        try_probe_directdraw_interfaces(direct_draw, "DirectDrawCreate");
        emit_directdraw_hook_status();
    }

    send_event(make_event(
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
    if hresult_succeeded(result) && !direct_draw_out.is_null() {
        let direct_draw = unsafe { *direct_draw_out };
        try_install_directdraw_object_hooks(direct_draw, "DirectDrawCreateEx");
        try_probe_directdraw_interfaces(direct_draw, "DirectDrawCreateEx");
        emit_directdraw_hook_status();
    }

    send_event(make_event(
        "DirectDrawCreateEx",
        format!(
            "guid_ptr={guid:p} out_ptr={direct_draw_out:p} iid_ptr={iid:p} outer_ptr={unknown_outer:p}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn co_create_instance_detour(
    rclsid: *const c_void,
    outer: *mut c_void,
    clsctx: u32,
    riid: *const c_void,
    ppv: *mut *mut c_void,
) -> i32 {
    let result = unsafe {
        CO_CREATE_INSTANCE_HOOK
            .get()
            .expect("CoCreateInstance hook not installed")
            .call(rclsid, outer, clsctx, riid, ppv)
    };

    let is_directdraw_request = guid_ptr_matches(rclsid, &CLSID_DIRECTDRAW)
        || guid_ptr_matches(rclsid, &CLSID_DIRECTDRAW7)
        || guid_ptr_matches(riid, &IID_IDIRECTDRAW)
        || guid_ptr_matches(riid, &IID_IDIRECTDRAW2)
        || guid_ptr_matches(riid, &IID_IDIRECTDRAW4)
        || guid_ptr_matches(riid, &IID_IDIRECTDRAW7);

    if is_directdraw_request {
        if hresult_succeeded(result) && !ppv.is_null() {
            let direct_draw = unsafe { *ppv };
            try_install_directdraw_object_hooks(direct_draw, "CoCreateInstance");
            try_probe_directdraw_interfaces(direct_draw, "CoCreateInstance");
            emit_directdraw_hook_status();
        }
        send_event(make_event(
            "CoCreateInstance(DirectDraw)",
            format!("rclsid={rclsid:p} riid={riid:p} clsctx=0x{clsctx:08X} out_ptr={ppv:p}"),
            hresult_result(result),
        ));
    }

    result
}

unsafe extern "system" fn co_create_instance_ex_detour(
    rclsid: *const c_void,
    outer: *mut c_void,
    clsctx: u32,
    server_info: *mut c_void,
    count: u32,
    results: *mut MultiQi,
) -> i32 {
    let result = unsafe {
        CO_CREATE_INSTANCE_EX_HOOK
            .get()
            .expect("CoCreateInstanceEx hook not installed")
            .call(rclsid, outer, clsctx, server_info, count, results)
    };

    let is_directdraw_request = guid_ptr_matches(rclsid, &CLSID_DIRECTDRAW)
        || guid_ptr_matches(rclsid, &CLSID_DIRECTDRAW7)
        || (count > 0
            && !results.is_null()
            && (0..count).any(|i| {
                let qi = unsafe { &*results.add(i as usize) };
                is_directdraw_interface_iid(qi.riid)
            }));

    if is_directdraw_request {
        if hresult_succeeded(result) && count > 0 && !results.is_null() {
            for i in 0..count {
                let qi = unsafe { &*results.add(i as usize) };
                if !is_directdraw_interface_iid(qi.riid) || !hresult_succeeded(qi.hr) {
                    continue;
                }
                if !qi.out_object.is_null() {
                    try_install_directdraw_object_hooks(qi.out_object, "CoCreateInstanceEx");
                    try_probe_directdraw_interfaces(qi.out_object, "CoCreateInstanceEx");
                }
            }
            emit_directdraw_hook_status();
        }

        send_event(make_event(
            "CoCreateInstanceEx(DirectDraw)",
            format!(
                "rclsid={rclsid:p} clsctx=0x{clsctx:08X} server_info={server_info:p} count={count} results_ptr={results:p}"
            ),
            hresult_result(result),
        ));
    }

    result
}

unsafe extern "system" fn dd_create_surface_detour(
    this: *mut c_void,
    surface_desc: *mut c_void,
    surface_out: *mut *mut c_void,
    unknown_outer: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_CREATE_SURFACE_HOOK
            .get()
            .expect("IDirectDraw::CreateSurface hook not installed")
            .call(this, surface_desc, surface_out, unknown_outer)
    };

    let out_value = if surface_out.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { *surface_out }
    };
    if hresult_succeeded(result) && !out_value.is_null() {
        try_install_directdraw_surface_hooks(out_value, "IDirectDraw::CreateSurface");
    }
    send_event(make_event(
        "IDirectDraw::CreateSurface",
        format!(
            "this={this:p} desc_ptr={surface_desc:p} ({}) out_ptr={surface_out:p} outer_ptr={unknown_outer:p}",
            describe_dd_surface_desc(surface_desc),
        ),
        format!("{} surface={out_value:p}", hresult_result(result)),
    ));
    result
}

unsafe extern "system" fn dd_create_surface_alt_detour(
    this: *mut c_void,
    surface_desc: *mut c_void,
    surface_out: *mut *mut c_void,
    unknown_outer: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_CREATE_SURFACE_ALT_HOOK
            .get()
            .expect("IDirectDraw::CreateSurface alt hook not installed")
            .call(this, surface_desc, surface_out, unknown_outer)
    };

    let out_value = if surface_out.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { *surface_out }
    };
    if hresult_succeeded(result) && !out_value.is_null() {
        try_install_directdraw_surface_hooks(out_value, "IDirectDraw::CreateSurface(alt)");
    }
    send_event(make_event(
        "IDirectDraw::CreateSurface",
        format!(
            "this={this:p} desc_ptr={surface_desc:p} ({}) out_ptr={surface_out:p} outer_ptr={unknown_outer:p}",
            describe_dd_surface_desc(surface_desc),
        ),
        format!("{} surface={out_value:p}", hresult_result(result)),
    ));
    result
}

unsafe extern "system" fn dd_set_cooperative_level_detour(
    this: *mut c_void,
    hwnd: isize,
    flags: u32,
) -> i32 {
    let result = unsafe {
        DD_SET_COOPERATIVE_LEVEL_HOOK
            .get()
            .expect("IDirectDraw::SetCooperativeLevel hook not installed")
            .call(this, hwnd, flags)
    };

    send_event(make_event(
        "IDirectDraw::SetCooperativeLevel",
        format!("this={this:p} hwnd=0x{hwnd:016X} flags=0x{flags:08X}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_set_cooperative_level_alt_detour(
    this: *mut c_void,
    hwnd: isize,
    flags: u32,
) -> i32 {
    let result = unsafe {
        DD_SET_COOPERATIVE_LEVEL_ALT_HOOK
            .get()
            .expect("IDirectDraw::SetCooperativeLevel alt hook not installed")
            .call(this, hwnd, flags)
    };

    send_event(make_event(
        "IDirectDraw::SetCooperativeLevel",
        format!("this={this:p} hwnd=0x{hwnd:016X} flags=0x{flags:08X}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_set_display_mode_detour(
    this: *mut c_void,
    width: u32,
    height: u32,
    bpp: u32,
) -> i32 {
    let result = unsafe {
        DD_SET_DISPLAY_MODE_HOOK
            .get()
            .expect("IDirectDraw::SetDisplayMode hook not installed")
            .call(this, width, height, bpp)
    };

    send_event(make_event(
        "IDirectDraw::SetDisplayMode",
        format!("this={this:p} width={width} height={height} bpp={bpp}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_set_display_mode_alt_detour(
    this: *mut c_void,
    width: u32,
    height: u32,
    bpp: u32,
) -> i32 {
    let result = unsafe {
        DD_SET_DISPLAY_MODE_ALT_HOOK
            .get()
            .expect("IDirectDraw::SetDisplayMode alt hook not installed")
            .call(this, width, height, bpp)
    };

    send_event(make_event(
        "IDirectDraw::SetDisplayMode",
        format!("this={this:p} width={width} height={height} bpp={bpp}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_set_display_mode_ex_detour(
    this: *mut c_void,
    width: u32,
    height: u32,
    bpp: u32,
    refresh_rate: u32,
    flags: u32,
) -> i32 {
    let result = unsafe {
        DD_SET_DISPLAY_MODE_EX_HOOK
            .get()
            .expect("IDirectDraw7::SetDisplayMode hook not installed")
            .call(this, width, height, bpp, refresh_rate, flags)
    };

    send_event(make_event(
        "IDirectDraw7::SetDisplayMode",
        format!(
            "this={this:p} width={width} height={height} bpp={bpp} refresh={refresh_rate} flags=0x{flags:08X}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_set_display_mode_ex_alt_detour(
    this: *mut c_void,
    width: u32,
    height: u32,
    bpp: u32,
    refresh_rate: u32,
    flags: u32,
) -> i32 {
    let result = unsafe {
        DD_SET_DISPLAY_MODE_EX_ALT_HOOK
            .get()
            .expect("IDirectDraw7::SetDisplayMode alt hook not installed")
            .call(this, width, height, bpp, refresh_rate, flags)
    };

    send_event(make_event(
        "IDirectDraw7::SetDisplayMode",
        format!(
            "this={this:p} width={width} height={height} bpp={bpp} refresh={refresh_rate} flags=0x{flags:08X}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_restore_display_mode_detour(this: *mut c_void) -> i32 {
    let result = unsafe {
        DD_RESTORE_DISPLAY_MODE_HOOK
            .get()
            .expect("IDirectDraw::RestoreDisplayMode hook not installed")
            .call(this)
    };

    send_event(make_event(
        "IDirectDraw::RestoreDisplayMode",
        format!("this={this:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_restore_display_mode_alt_detour(this: *mut c_void) -> i32 {
    let result = unsafe {
        DD_RESTORE_DISPLAY_MODE_ALT_HOOK
            .get()
            .expect("IDirectDraw::RestoreDisplayMode alt hook not installed")
            .call(this)
    };

    send_event(make_event(
        "IDirectDraw::RestoreDisplayMode",
        format!("this={this:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_wait_for_vblank_detour(
    this: *mut c_void,
    flags: u32,
    event: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_WAIT_FOR_VBLANK_HOOK
            .get()
            .expect("IDirectDraw::WaitForVerticalBlank hook not installed")
            .call(this, flags, event)
    };

    send_event(make_event(
        "IDirectDraw::WaitForVerticalBlank",
        format!("this={this:p} flags=0x{flags:08X} event={event:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_wait_for_vblank_alt_detour(
    this: *mut c_void,
    flags: u32,
    event: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_WAIT_FOR_VBLANK_ALT_HOOK
            .get()
            .expect("IDirectDraw::WaitForVerticalBlank alt hook not installed")
            .call(this, flags, event)
    };

    send_event(make_event(
        "IDirectDraw::WaitForVerticalBlank",
        format!("this={this:p} flags=0x{flags:08X} event={event:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_blt_detour(
    this: *mut c_void,
    dst_rect: *mut RECT,
    src_surface: *mut c_void,
    src_rect: *mut RECT,
    flags: u32,
    fx: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_BLT_HOOK
            .get()
            .expect("IDirectDrawSurface::Blt hook not installed")
            .call(this, dst_rect, src_surface, src_rect, flags, fx)
    };
    send_event(make_event(
        "IDirectDrawSurface::Blt",
        format!(
            "this={this:p} dst_rect={dst_rect:p} src_surface={src_surface:p} src_rect={src_rect:p} flags=0x{flags:08X} fx={fx:p}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_blt_alt_detour(
    this: *mut c_void,
    dst_rect: *mut RECT,
    src_surface: *mut c_void,
    src_rect: *mut RECT,
    flags: u32,
    fx: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_BLT_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::Blt alt hook not installed")
            .call(this, dst_rect, src_surface, src_rect, flags, fx)
    };
    send_event(make_event(
        "IDirectDrawSurface::Blt",
        format!(
            "this={this:p} dst_rect={dst_rect:p} src_surface={src_surface:p} src_rect={src_rect:p} flags=0x{flags:08X} fx={fx:p}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_bltfast_detour(
    this: *mut c_void,
    x: u32,
    y: u32,
    src_surface: *mut c_void,
    src_rect: *mut RECT,
    trans: u32,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_BLTFAST_HOOK
            .get()
            .expect("IDirectDrawSurface::BltFast hook not installed")
            .call(this, x, y, src_surface, src_rect, trans)
    };
    send_event(make_event(
        "IDirectDrawSurface::BltFast",
        format!(
            "this={this:p} x={x} y={y} src_surface={src_surface:p} src_rect={src_rect:p} trans=0x{trans:08X}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_bltfast_alt_detour(
    this: *mut c_void,
    x: u32,
    y: u32,
    src_surface: *mut c_void,
    src_rect: *mut RECT,
    trans: u32,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_BLTFAST_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::BltFast alt hook not installed")
            .call(this, x, y, src_surface, src_rect, trans)
    };
    send_event(make_event(
        "IDirectDrawSurface::BltFast",
        format!(
            "this={this:p} x={x} y={y} src_surface={src_surface:p} src_rect={src_rect:p} trans=0x{trans:08X}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_flip_detour(
    this: *mut c_void,
    target_override: *mut c_void,
    flags: u32,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_FLIP_HOOK
            .get()
            .expect("IDirectDrawSurface::Flip hook not installed")
            .call(this, target_override, flags)
    };
    send_event(make_event(
        "IDirectDrawSurface::Flip",
        format!("this={this:p} target_override={target_override:p} flags=0x{flags:08X}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_flip_alt_detour(
    this: *mut c_void,
    target_override: *mut c_void,
    flags: u32,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_FLIP_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::Flip alt hook not installed")
            .call(this, target_override, flags)
    };
    send_event(make_event(
        "IDirectDrawSurface::Flip",
        format!("this={this:p} target_override={target_override:p} flags=0x{flags:08X}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_getdc_detour(this: *mut c_void, hdc_out: *mut isize) -> i32 {
    let result = unsafe {
        DD_SURFACE_GETDC_HOOK
            .get()
            .expect("IDirectDrawSurface::GetDC hook not installed")
            .call(this, hdc_out)
    };

    let out_value = if hdc_out.is_null() {
        0usize
    } else {
        unsafe { *hdc_out as usize }
    };
    send_event(make_event(
        "IDirectDrawSurface::GetDC",
        format!("this={this:p} out_ptr={hdc_out:p} out=0x{out_value:016X}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_getdc_alt_detour(
    this: *mut c_void,
    hdc_out: *mut isize,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_GETDC_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::GetDC alt hook not installed")
            .call(this, hdc_out)
    };

    let out_value = if hdc_out.is_null() {
        0usize
    } else {
        unsafe { *hdc_out as usize }
    };
    send_event(make_event(
        "IDirectDrawSurface::GetDC",
        format!("this={this:p} out_ptr={hdc_out:p} out=0x{out_value:016X}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_lock_detour(
    this: *mut c_void,
    rect: *mut RECT,
    desc: *mut c_void,
    flags: u32,
    handle: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_LOCK_HOOK
            .get()
            .expect("IDirectDrawSurface::Lock hook not installed")
            .call(this, rect, desc, flags, handle)
    };
    send_event(make_event(
        "IDirectDrawSurface::Lock",
        format!("this={this:p} rect={rect:p} desc={desc:p} flags=0x{flags:08X} handle={handle:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_lock_alt_detour(
    this: *mut c_void,
    rect: *mut RECT,
    desc: *mut c_void,
    flags: u32,
    handle: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_LOCK_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::Lock alt hook not installed")
            .call(this, rect, desc, flags, handle)
    };
    send_event(make_event(
        "IDirectDrawSurface::Lock",
        format!("this={this:p} rect={rect:p} desc={desc:p} flags=0x{flags:08X} handle={handle:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_unlock_detour(this: *mut c_void, data: *mut c_void) -> i32 {
    let result = unsafe {
        DD_SURFACE_UNLOCK_HOOK
            .get()
            .expect("IDirectDrawSurface::Unlock hook not installed")
            .call(this, data)
    };
    send_event(make_event(
        "IDirectDrawSurface::Unlock",
        format!("this={this:p} data={data:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_unlock_alt_detour(
    this: *mut c_void,
    data: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_UNLOCK_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::Unlock alt hook not installed")
            .call(this, data)
    };
    send_event(make_event(
        "IDirectDrawSurface::Unlock",
        format!("this={this:p} data={data:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_releasedc_detour(this: *mut c_void, hdc: isize) -> i32 {
    let result = unsafe {
        DD_SURFACE_RELEASEDC_HOOK
            .get()
            .expect("IDirectDrawSurface::ReleaseDC hook not installed")
            .call(this, hdc)
    };
    send_event(make_event(
        "IDirectDrawSurface::ReleaseDC",
        format!("this={this:p} hdc=0x{:016X}", hdc as usize),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_releasedc_alt_detour(this: *mut c_void, hdc: isize) -> i32 {
    let result = unsafe {
        DD_SURFACE_RELEASEDC_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::ReleaseDC alt hook not installed")
            .call(this, hdc)
    };
    send_event(make_event(
        "IDirectDrawSurface::ReleaseDC",
        format!("this={this:p} hdc=0x{:016X}", hdc as usize),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_islost_detour(this: *mut c_void) -> i32 {
    let result = unsafe {
        DD_SURFACE_ISLOST_HOOK
            .get()
            .expect("IDirectDrawSurface::IsLost hook not installed")
            .call(this)
    };
    send_event(make_event(
        "IDirectDrawSurface::IsLost",
        format!("this={this:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_islost_alt_detour(this: *mut c_void) -> i32 {
    let result = unsafe {
        DD_SURFACE_ISLOST_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::IsLost alt hook not installed")
            .call(this)
    };
    send_event(make_event(
        "IDirectDrawSurface::IsLost",
        format!("this={this:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_restore_detour(this: *mut c_void) -> i32 {
    let result = unsafe {
        DD_SURFACE_RESTORE_HOOK
            .get()
            .expect("IDirectDrawSurface::Restore hook not installed")
            .call(this)
    };
    send_event(make_event(
        "IDirectDrawSurface::Restore",
        format!("this={this:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_restore_alt_detour(this: *mut c_void) -> i32 {
    let result = unsafe {
        DD_SURFACE_RESTORE_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::Restore alt hook not installed")
            .call(this)
    };
    send_event(make_event(
        "IDirectDrawSurface::Restore",
        format!("this={this:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_getdesc_detour(this: *mut c_void, desc: *mut c_void) -> i32 {
    let result = unsafe {
        DD_SURFACE_GETDESC_HOOK
            .get()
            .expect("IDirectDrawSurface::GetSurfaceDesc hook not installed")
            .call(this, desc)
    };
    send_event(make_event(
        "IDirectDrawSurface::GetSurfaceDesc",
        format!("this={this:p} desc_ptr={desc:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_getdesc_alt_detour(
    this: *mut c_void,
    desc: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_GETDESC_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::GetSurfaceDesc alt hook not installed")
            .call(this, desc)
    };
    send_event(make_event(
        "IDirectDrawSurface::GetSurfaceDesc",
        format!("this={this:p} desc_ptr={desc:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_getattached_detour(
    this: *mut c_void,
    caps: *mut c_void,
    attached_out: *mut *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_GETATTACHED_HOOK
            .get()
            .expect("IDirectDrawSurface::GetAttachedSurface hook not installed")
            .call(this, caps, attached_out)
    };

    let out_value = if attached_out.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { *attached_out }
    };

    if hresult_succeeded(result) && !out_value.is_null() {
        try_install_directdraw_surface_hooks(out_value, "IDirectDrawSurface::GetAttachedSurface");
    }

    send_event(make_event(
        "IDirectDrawSurface::GetAttachedSurface",
        format!("this={this:p} caps={caps:p} out_ptr={attached_out:p} out={out_value:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_getattached_alt_detour(
    this: *mut c_void,
    caps: *mut c_void,
    attached_out: *mut *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_GETATTACHED_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::GetAttachedSurface alt hook not installed")
            .call(this, caps, attached_out)
    };

    let out_value = if attached_out.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { *attached_out }
    };

    if hresult_succeeded(result) && !out_value.is_null() {
        try_install_directdraw_surface_hooks(
            out_value,
            "IDirectDrawSurface::GetAttachedSurface(alt)",
        );
    }

    send_event(make_event(
        "IDirectDrawSurface::GetAttachedSurface",
        format!("this={this:p} caps={caps:p} out_ptr={attached_out:p} out={out_value:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_setclipper_detour(
    this: *mut c_void,
    clipper: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_SETCLIPPER_HOOK
            .get()
            .expect("IDirectDrawSurface::SetClipper hook not installed")
            .call(this, clipper)
    };
    send_event(make_event(
        "IDirectDrawSurface::SetClipper",
        format!("this={this:p} clipper={clipper:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_setclipper_alt_detour(
    this: *mut c_void,
    clipper: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_SETCLIPPER_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::SetClipper alt hook not installed")
            .call(this, clipper)
    };
    send_event(make_event(
        "IDirectDrawSurface::SetClipper",
        format!("this={this:p} clipper={clipper:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_setpalette_detour(
    this: *mut c_void,
    palette: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_SETPALETTE_HOOK
            .get()
            .expect("IDirectDrawSurface::SetPalette hook not installed")
            .call(this, palette)
    };
    send_event(make_event(
        "IDirectDrawSurface::SetPalette",
        format!("this={this:p} palette={palette:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_surface_setpalette_alt_detour(
    this: *mut c_void,
    palette: *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_SURFACE_SETPALETTE_ALT_HOOK
            .get()
            .expect("IDirectDrawSurface::SetPalette alt hook not installed")
            .call(this, palette)
    };
    send_event(make_event(
        "IDirectDrawSurface::SetPalette",
        format!("this={this:p} palette={palette:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn dd_query_interface_detour(
    this: *mut c_void,
    riid: *const c_void,
    out_object: *mut *mut c_void,
) -> i32 {
    let result = unsafe {
        DD_QUERY_INTERFACE_HOOK
            .get()
            .expect("IDirectDraw::QueryInterface hook not installed")
            .call(this, riid, out_object)
    };

    let out_value = if out_object.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { *out_object }
    };
    let is_directdraw_iid = is_directdraw_interface_iid(riid);
    if is_directdraw_iid && hresult_succeeded(result) && !out_value.is_null() {
        try_install_directdraw_object_hooks(out_value, "IDirectDraw::QueryInterface");
        emit_directdraw_hook_status();
    }

    send_event(make_event(
        "IDirectDraw::QueryInterface",
        format!(
            "this={this:p} riid={riid:p} out_ptr={out_object:p} out={out_value:p} directdraw_iid={is_directdraw_iid}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn directdraw_create_clipper_detour(
    flags: u32,
    clipper_out: *mut *mut c_void,
    unknown_outer: *mut c_void,
) -> i32 {
    let result = unsafe {
        DIRECTDRAW_CREATE_CLIPPER_HOOK
            .get()
            .expect("DirectDrawCreateClipper hook not installed")
            .call(flags, clipper_out, unknown_outer)
    };

    send_event(make_event(
        "DirectDrawCreateClipper",
        format!("flags=0x{flags:08X} out_ptr={clipper_out:p} outer_ptr={unknown_outer:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn directdraw_enumerate_a_detour(
    callback: *mut c_void,
    context: *mut c_void,
) -> i32 {
    let result = unsafe {
        DIRECTDRAW_ENUMERATE_A_HOOK
            .get()
            .expect("DirectDrawEnumerateA hook not installed")
            .call(callback, context)
    };

    send_event(make_event(
        "DirectDrawEnumerateA",
        format!("callback={callback:p} context={context:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn directdraw_enumerate_w_detour(
    callback: *mut c_void,
    context: *mut c_void,
) -> i32 {
    let result = unsafe {
        DIRECTDRAW_ENUMERATE_W_HOOK
            .get()
            .expect("DirectDrawEnumerateW hook not installed")
            .call(callback, context)
    };

    send_event(make_event(
        "DirectDrawEnumerateW",
        format!("callback={callback:p} context={context:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn directdraw_enumerate_ex_a_detour(
    callback: *mut c_void,
    context: *mut c_void,
    flags: u32,
) -> i32 {
    let result = unsafe {
        DIRECTDRAW_ENUMERATE_EX_A_HOOK
            .get()
            .expect("DirectDrawEnumerateExA hook not installed")
            .call(callback, context, flags)
    };

    send_event(make_event(
        "DirectDrawEnumerateExA",
        format!("callback={callback:p} context={context:p} flags=0x{flags:08X}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn directdraw_enumerate_ex_w_detour(
    callback: *mut c_void,
    context: *mut c_void,
    flags: u32,
) -> i32 {
    let result = unsafe {
        DIRECTDRAW_ENUMERATE_EX_W_HOOK
            .get()
            .expect("DirectDrawEnumerateExW hook not installed")
            .call(callback, context, flags)
    };

    send_event(make_event(
        "DirectDrawEnumerateExW",
        format!("callback={callback:p} context={context:p} flags=0x{flags:08X}"),
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

    send_event(make_event(
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

    send_event(make_event(
        "Direct3DCreate9Ex",
        format!("sdk_version={sdk_version} out_ptr={direct3d_out:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn create_dxgi_factory_detour(
    iid: *const c_void,
    factory_out: *mut *mut c_void,
) -> i32 {
    let result = unsafe {
        CREATE_DXGI_FACTORY_HOOK
            .get()
            .expect("CreateDXGIFactory hook not installed")
            .call(iid, factory_out)
    };

    send_event(make_event(
        "CreateDXGIFactory",
        format!("iid_ptr={iid:p} out_ptr={factory_out:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn create_dxgi_factory1_detour(
    iid: *const c_void,
    factory_out: *mut *mut c_void,
) -> i32 {
    let result = unsafe {
        CREATE_DXGI_FACTORY1_HOOK
            .get()
            .expect("CreateDXGIFactory1 hook not installed")
            .call(iid, factory_out)
    };

    send_event(make_event(
        "CreateDXGIFactory1",
        format!("iid_ptr={iid:p} out_ptr={factory_out:p}"),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn d3d11_create_device_detour(
    adapter: *mut c_void,
    driver_type: u32,
    software: *mut c_void,
    flags: u32,
    feature_levels: *const u32,
    feature_levels_count: u32,
    sdk_version: u32,
    device_out: *mut *mut c_void,
    feature_level_out: *mut u32,
    context_out: *mut *mut c_void,
) -> i32 {
    let result = unsafe {
        D3D11_CREATE_DEVICE_HOOK
            .get()
            .expect("D3D11CreateDevice hook not installed")
            .call(
                adapter,
                driver_type,
                software,
                flags,
                feature_levels,
                feature_levels_count,
                sdk_version,
                device_out,
                feature_level_out,
                context_out,
            )
    };

    send_event(make_event(
        "D3D11CreateDevice",
        format!(
            "adapter={adapter:p} driver_type={driver_type} flags=0x{flags:08X} feature_count={feature_levels_count} sdk={sdk_version}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn d3d11_create_device_and_swap_chain_detour(
    adapter: *mut c_void,
    driver_type: u32,
    software: *mut c_void,
    flags: u32,
    feature_levels: *const u32,
    feature_levels_count: u32,
    sdk_version: u32,
    swap_chain_desc: *const c_void,
    swap_chain_out: *mut *mut c_void,
    device_out: *mut *mut c_void,
    feature_level_out: *mut u32,
    context_out: *mut *mut c_void,
) -> i32 {
    let result = unsafe {
        D3D11_CREATE_DEVICE_AND_SWAP_CHAIN_HOOK
            .get()
            .expect("D3D11CreateDeviceAndSwapChain hook not installed")
            .call(
                adapter,
                driver_type,
                software,
                flags,
                feature_levels,
                feature_levels_count,
                sdk_version,
                swap_chain_desc,
                swap_chain_out,
                device_out,
                feature_level_out,
                context_out,
            )
    };

    send_event(make_event(
        "D3D11CreateDeviceAndSwapChain",
        format!(
            "adapter={adapter:p} driver_type={driver_type} flags=0x{flags:08X} feature_count={feature_levels_count} sdk={sdk_version} swap_desc={swap_chain_desc:p}"
        ),
        hresult_result(result),
    ));
    result
}

unsafe extern "system" fn load_library_a_detour(library_file_name: *const u8) -> *mut c_void {
    let requested = read_c_string_lossy(library_file_name);
    let module = unsafe {
        LOAD_LIBRARY_A_HOOK
            .get()
            .expect("LoadLibraryA hook not installed")
            .call(library_file_name)
    };

    emit_dll_load("LoadLibraryA", &requested, module);
    let _ = try_install_optional_graphics_hooks();
    module
}

unsafe extern "system" fn load_library_w_detour(library_file_name: *const u16) -> *mut c_void {
    let requested = read_wide_string_lossy(library_file_name);
    let module = unsafe {
        LOAD_LIBRARY_W_HOOK
            .get()
            .expect("LoadLibraryW hook not installed")
            .call(library_file_name)
    };

    emit_dll_load("LoadLibraryW", &requested, module);
    let _ = try_install_optional_graphics_hooks();
    module
}

unsafe extern "system" fn load_library_ex_a_detour(
    library_file_name: *const u8,
    file: *mut c_void,
    flags: u32,
) -> *mut c_void {
    let requested = read_c_string_lossy(library_file_name);
    let module = unsafe {
        LOAD_LIBRARY_EX_A_HOOK
            .get()
            .expect("LoadLibraryExA hook not installed")
            .call(library_file_name, file, flags)
    };

    emit_dll_load(
        &format!("LoadLibraryExA flags=0x{flags:08X}"),
        &requested,
        module,
    );
    let _ = try_install_optional_graphics_hooks();
    module
}

unsafe extern "system" fn load_library_ex_w_detour(
    library_file_name: *const u16,
    file: *mut c_void,
    flags: u32,
) -> *mut c_void {
    let requested = read_wide_string_lossy(library_file_name);
    let module = unsafe {
        LOAD_LIBRARY_EX_W_HOOK
            .get()
            .expect("LoadLibraryExW hook not installed")
            .call(library_file_name, file, flags)
    };

    emit_dll_load(
        &format!("LoadLibraryExW flags=0x{flags:08X}"),
        &requested,
        module,
    );
    let _ = try_install_optional_graphics_hooks();
    module
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

fn emit_dll_load(source: &str, requested: &str, module: *mut c_void) {
    let path = get_module_file_name_lossy(module);
    let summary = if requested.is_empty() {
        format!("{source} module={module:p}")
    } else {
        format!("{source} module={module:p} requested=\"{requested}\"")
    };

    let result = if module.is_null() {
        "(failed)".to_owned()
    } else if path.is_empty() {
        "(path unavailable)".to_owned()
    } else {
        path
    };

    // DLL load logging can be extremely noisy (and we transport over UDP). De-dupe by "best effort"
    // key so it doesn't drown out higher-signal DirectDraw events.
    let key = if result != "(failed)" && result != "(path unavailable)" && !result.is_empty() {
        result.clone()
    } else if !requested.is_empty() {
        requested.to_owned()
    } else {
        summary.clone()
    };

    let set = DLL_LOAD_KEYS.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut guard) = set.lock() {
        if !guard.insert(key) {
            return;
        }
    }

    send_event(make_event("DllLoad", summary, result));
}

fn get_module_file_name_lossy(module: *mut c_void) -> String {
    if module.is_null() {
        return String::new();
    }

    let mut buffer = vec![0u16; 1024];
    let len = unsafe { GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32) };
    if len == 0 {
        return String::new();
    }

    buffer.truncate(len as usize);
    String::from_utf16_lossy(&buffer)
}

fn read_c_string_lossy(ptr: *const u8) -> String {
    if ptr.is_null() {
        return String::new();
    }

    // Avoid over-reading if something goes wrong; DLL names are short.
    let mut bytes = Vec::with_capacity(260);
    for i in 0..260usize {
        let p = unsafe { ptr.add(i) } as *const c_void;
        if !is_readable_ptr(p, 1) {
            break;
        }
        let b = unsafe { *(ptr.add(i)) };
        if b == 0 {
            break;
        }
        bytes.push(b);
    }

    String::from_utf8_lossy(&bytes).to_string()
}

fn read_wide_string_lossy(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }

    let mut chars: Vec<u16> = Vec::with_capacity(260);
    for i in 0..260usize {
        let p = unsafe { ptr.add(i) } as *const c_void;
        if !is_readable_ptr(p, 2) {
            break;
        }
        let w = unsafe { *(ptr.add(i)) };
        if w == 0 {
            break;
        }
        chars.push(w);
    }

    String::from_utf16_lossy(&chars)
}

fn emit_loaded_modules_snapshot() {
    let pid = unsafe { GetCurrentProcessId() };
    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
    if snapshot == INVALID_HANDLE_VALUE {
        return;
    }

    let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;

    let ok = unsafe { Module32FirstW(snapshot, &mut entry as *mut MODULEENTRY32W) };
    if ok == 0 {
        unsafe {
            CloseHandle(snapshot);
        }
        return;
    }

    loop {
        let module = entry.hModule as *mut c_void;
        let name = utf16z_to_string(&entry.szModule);
        let path = utf16z_to_string(&entry.szExePath);
        if should_emit_snapshot_module(&name) {
            let summary = format!("Snapshot module={module:p} name=\"{name}\"");
            let result = if path.is_empty() {
                "(path unavailable)".to_owned()
            } else {
                path
            };
            send_event(make_event("DllLoad", summary, result));
        }

        let next = unsafe { Module32NextW(snapshot, &mut entry as *mut MODULEENTRY32W) };
        if next == 0 {
            break;
        }
    }

    unsafe {
        CloseHandle(snapshot);
    }
}

fn should_emit_snapshot_module(name: &str) -> bool {
    // Avoid flooding UDP on attach; we only snapshot modules that are relevant for graphics/COM
    // troubleshooting. Runtime LoadLibrary* events will still get logged for everything.
    let n = name.to_ascii_lowercase();
    matches!(
        n.as_str(),
        "ddraw.dll"
            | "ddrawex.dll"
            | "ole32.dll"
            | "user32.dll"
            | "kernel32.dll"
            | "gdi32.dll"
            | "opengl32.dll"
            | "dxgi.dll"
            | "d3d9.dll"
            | "d3d11.dll"
    ) || n.starts_with("d3dcompiler_")
}

fn utf16z_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn emit_agent_build() {
    let arch = if cfg!(target_pointer_width = "32") {
        "x86"
    } else if cfg!(target_pointer_width = "64") {
        "x64"
    } else {
        "unknown"
    };
    send_event(make_event(
        "AgentBuild",
        format!(
            "tag={AGENT_BUILD_TAG} version={}",
            env!("CARGO_PKG_VERSION")
        ),
        format!("arch={arch}"),
    ));
}

fn try_probe_install_directdraw_vtable_hooks() {
    // If we injected after the target already initialized DirectDraw, we might never see
    // DirectDrawCreate/Ex, which means we won't learn the COM vtable method addresses.
    //
    // This probe creates a temporary DirectDraw object (if possible) and uses it to install the
    // typed vtable hooks (CreateSurface/SetCooperativeLevel/SetDisplayMode*). These detours are
    // global by address, so they can still catch "real usage" in the target afterwards.
    if DD_CREATE_SURFACE_HOOK.get().is_some() || DD_CREATE_SURFACE_ALT_HOOK.get().is_some() {
        return;
    }

    let ddraw_loaded = unsafe { GetModuleHandleA(b"ddraw.dll\0".as_ptr()) };
    if ddraw_loaded.is_null() {
        return;
    }

    let mut out: *mut c_void = std::ptr::null_mut();
    let mut hr: i32 = -1;

    if let Some(h) = DIRECTDRAW_CREATE_EX_HOOK.get() {
        hr = unsafe {
            h.call(
                std::ptr::null(),
                &mut out as *mut *mut c_void,
                (&IID_IDIRECTDRAW7 as *const Guid) as *const c_void,
                std::ptr::null_mut(),
            )
        };
    } else if let Some(h) = DIRECTDRAW_CREATE_HOOK.get() {
        hr = unsafe {
            h.call(
                std::ptr::null(),
                &mut out as *mut *mut c_void,
                std::ptr::null_mut(),
            )
        };
    }

    if hresult_succeeded(hr) && !out.is_null() {
        // Install vtable hooks from this object, then release it.
        try_install_directdraw_object_hooks(out, "DirectDrawProbe");
        try_probe_directdraw_interfaces(out, "DirectDrawProbe");
        call_directdraw_release(out);
        send_event(make_event(
            "DirectDrawProbe",
            "Temporary DirectDraw object created; vtable hooks installed".to_owned(),
            hresult_result(hr),
        ));
    } else {
        send_event(make_event(
            "DirectDrawProbe",
            "Failed to create temporary DirectDraw object (likely injected very late or runtime blocks it)"
                .to_owned(),
            hresult_result(hr),
        ));
    }
}

fn emit_directdraw_hook_status() {
    let ddraw_loaded = unsafe { GetModuleHandleA(b"ddraw.dll\0".as_ptr()) };
    let ddrawex_loaded = unsafe { GetModuleHandleA(b"ddrawex.dll\0".as_ptr()) };
    let ole32_loaded = unsafe { GetModuleHandleA(b"ole32.dll\0".as_ptr()) };
    let summary = format!(
        "ddraw_loaded={} ddrawex_loaded={} ole32_loaded={} create={} create_ex={} clipper={} create_surface={}",
        !ddraw_loaded.is_null(),
        !ddrawex_loaded.is_null(),
        !ole32_loaded.is_null(),
        DIRECTDRAW_CREATE_HOOK.get().is_some(),
        DIRECTDRAW_CREATE_EX_HOOK.get().is_some(),
        DIRECTDRAW_CREATE_CLIPPER_HOOK.get().is_some(),
        DD_CREATE_SURFACE_HOOK.get().is_some() || DD_CREATE_SURFACE_ALT_HOOK.get().is_some(),
    );
    let result = format!(
        "enum_a={} enum_w={} enum_ex_a={} enum_ex_w={} cocreate={} cocreate_ex={} set_coop={} set_mode={} surf_blt={} surf_flip={} surf_dc={} surf_lock={}",
        DIRECTDRAW_ENUMERATE_A_HOOK.get().is_some(),
        DIRECTDRAW_ENUMERATE_W_HOOK.get().is_some(),
        DIRECTDRAW_ENUMERATE_EX_A_HOOK.get().is_some(),
        DIRECTDRAW_ENUMERATE_EX_W_HOOK.get().is_some(),
        CO_CREATE_INSTANCE_HOOK.get().is_some(),
        CO_CREATE_INSTANCE_EX_HOOK.get().is_some(),
        DD_SET_COOPERATIVE_LEVEL_HOOK.get().is_some()
            || DD_SET_COOPERATIVE_LEVEL_ALT_HOOK.get().is_some(),
        DD_SET_DISPLAY_MODE_HOOK.get().is_some()
            || DD_SET_DISPLAY_MODE_ALT_HOOK.get().is_some()
            || DD_SET_DISPLAY_MODE_EX_HOOK.get().is_some()
            || DD_SET_DISPLAY_MODE_EX_ALT_HOOK.get().is_some(),
        DD_SURFACE_BLT_HOOK.get().is_some()
            || DD_SURFACE_BLT_ALT_HOOK.get().is_some()
            || DD_SURFACE_BLTFAST_HOOK.get().is_some()
            || DD_SURFACE_BLTFAST_ALT_HOOK.get().is_some(),
        DD_SURFACE_FLIP_HOOK.get().is_some() || DD_SURFACE_FLIP_ALT_HOOK.get().is_some(),
        DD_SURFACE_GETDC_HOOK.get().is_some()
            || DD_SURFACE_GETDC_ALT_HOOK.get().is_some()
            || DD_SURFACE_RELEASEDC_HOOK.get().is_some()
            || DD_SURFACE_RELEASEDC_ALT_HOOK.get().is_some(),
        DD_SURFACE_LOCK_HOOK.get().is_some()
            || DD_SURFACE_LOCK_ALT_HOOK.get().is_some()
            || DD_SURFACE_UNLOCK_HOOK.get().is_some()
            || DD_SURFACE_UNLOCK_ALT_HOOK.get().is_some(),
    );
    send_event(make_event("DirectDrawHookStatus", summary, result));
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

fn hresult_succeeded(value: i32) -> bool {
    value >= 0
}

fn guid_ptr_matches(ptr: *const c_void, expected: &Guid) -> bool {
    if !is_readable_ptr(ptr, std::mem::size_of::<Guid>()) {
        return false;
    }

    let candidate = unsafe { std::ptr::read_unaligned(ptr as *const Guid) };
    candidate == *expected
}

fn is_directdraw_interface_iid(iid: *const c_void) -> bool {
    guid_ptr_matches(iid, &IID_IDIRECTDRAW)
        || guid_ptr_matches(iid, &IID_IDIRECTDRAW2)
        || guid_ptr_matches(iid, &IID_IDIRECTDRAW4)
        || guid_ptr_matches(iid, &IID_IDIRECTDRAW7)
}

fn source_is_directdraw_legacy_interface(source: &str) -> bool {
    source == "DirectDrawCreate"
        || source.ends_with("->IDirectDraw")
        || source.ends_with("IDirectDraw::QueryInterface")
}

fn source_is_directdraw_extended_interface(source: &str) -> bool {
    source.contains("DirectDrawCreateEx")
        || source.ends_with("->IDirectDraw2")
        || source.ends_with("->IDirectDraw4")
        || source.ends_with("->IDirectDraw7")
}

fn try_read_u32_at(ptr: *const c_void, offset: usize) -> Option<u32> {
    if ptr.is_null() {
        return None;
    }
    let addr = (ptr as usize).checked_add(offset)? as *const c_void;
    if !is_readable_ptr(addr, std::mem::size_of::<u32>()) {
        return None;
    }

    Some(unsafe { std::ptr::read_unaligned(addr as *const u32) })
}

fn describe_dd_surface_desc(surface_desc: *mut c_void) -> String {
    if surface_desc.is_null() {
        return "desc=null".to_owned();
    }

    let size = try_read_u32_at(surface_desc as *const c_void, 0);
    let flags = try_read_u32_at(surface_desc as *const c_void, 4);
    let height = try_read_u32_at(surface_desc as *const c_void, 8);
    let width = try_read_u32_at(surface_desc as *const c_void, 12);
    match (size, flags, width, height) {
        (Some(size), Some(flags), Some(width), Some(height)) => {
            format!("size={size} flags=0x{flags:08X} width={width} height={height}")
        }
        _ => "desc=unreadable".to_owned(),
    }
}

fn is_readable_ptr(ptr: *const c_void, bytes: usize) -> bool {
    if ptr.is_null() {
        return false;
    }

    let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let queried = unsafe {
        VirtualQuery(
            ptr,
            &mut mbi as *mut MEMORY_BASIC_INFORMATION,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if queried == 0 {
        return false;
    }

    if mbi.State != MEM_COMMIT {
        return false;
    }

    if (mbi.Protect & PAGE_NOACCESS) != 0 || (mbi.Protect & PAGE_GUARD) != 0 {
        return false;
    }

    let region_start = mbi.BaseAddress as usize;
    let region_end = region_start.saturating_add(mbi.RegionSize);
    let ptr_start = ptr as usize;
    let ptr_end = ptr_start.saturating_add(bytes);
    ptr_start >= region_start && ptr_end <= region_end
}

fn is_executable_ptr(ptr: *const c_void) -> bool {
    if ptr.is_null() {
        return false;
    }

    let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let queried = unsafe {
        VirtualQuery(
            ptr,
            &mut mbi as *mut MEMORY_BASIC_INFORMATION,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if queried == 0 || mbi.State != MEM_COMMIT {
        return false;
    }

    let protect = mbi.Protect;
    let exec = protect == PAGE_EXECUTE
        || protect == PAGE_EXECUTE_READ
        || protect == PAGE_EXECUTE_READWRITE
        || protect == PAGE_EXECUTE_WRITECOPY;
    exec && (protect & PAGE_GUARD) == 0 && (protect & PAGE_NOACCESS) == 0
}

fn vtable_method_ptr(instance: *mut c_void, method_index: usize) -> Option<*const c_void> {
    if !is_readable_ptr(
        instance as *const c_void,
        std::mem::size_of::<*const c_void>(),
    ) {
        return None;
    }

    let vtable = unsafe { *(instance as *const *const *const c_void) };
    if !is_readable_ptr(
        vtable as *const c_void,
        (method_index + 1) * std::mem::size_of::<*const c_void>(),
    ) {
        return None;
    }

    let method = unsafe { *vtable.add(method_index) };
    if !is_executable_ptr(method) {
        return None;
    }

    Some(method)
}

fn is_ptr_in_module(ptr: *const c_void, module_name: &'static [u8]) -> bool {
    if ptr.is_null() {
        return false;
    }

    let module = unsafe { GetModuleHandleA(module_name.as_ptr()) };
    if module.is_null() {
        return false;
    }

    let mut mbi: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let queried = unsafe {
        VirtualQuery(
            ptr,
            &mut mbi as *mut MEMORY_BASIC_INFORMATION,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if queried == 0 {
        return false;
    }

    mbi.AllocationBase == module as *mut c_void
}

fn is_ptr_in_directdraw_runtime(ptr: *const c_void) -> bool {
    is_ptr_in_module(ptr, b"ddraw.dll\0") || is_ptr_in_module(ptr, b"ddrawex.dll\0")
}

#[cfg(target_pointer_width = "32")]
unsafe extern "system" fn ddraw_vtable_usage_logger(kind: u32, index: u32, target: *const c_void) {
    let ok = DIRECTDRAW_VTABLE_USAGE_REPORTED
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::Relaxed)
        .is_ok();
    if !ok {
        return;
    }

    let kind_name = match kind {
        1 => "IDirectDraw*",
        2 => "IDirectDrawSurface*",
        _ => "IUnknown*",
    };
    send_event(make_event(
        "DirectDrawUsed",
        format!("kind={kind_name} vtbl_index={index} target={target:p}"),
        "ONCE".to_owned(),
    ));
}

#[cfg(target_pointer_width = "32")]
fn alloc_x86_ddraw_usage_stub(kind: u32, index: u32, target: *const c_void) -> Option<*mut c_void> {
    let flag_addr = (&DIRECTDRAW_VTABLE_USAGE_REPORTED as *const AtomicU8) as usize as u32;
    let target_addr = target as usize as u32;
    let logger_addr = ddraw_vtable_usage_logger as *const () as usize as u32;

    // x86 stub:
    //   cmp byte ptr [flag], 0
    //   jne tailjmp
    //   pushfd; pushad
    //   push target; push index; push kind
    //   call logger
    //   popad; popfd
    //   mov byte ptr [flag], 1
    // tailjmp:
    //   mov eax, target
    //   jmp eax
    let mut code: Vec<u8> = Vec::with_capacity(64);

    // cmp byte ptr [flag_addr], 0
    code.extend([0x80, 0x3D]);
    code.extend(flag_addr.to_le_bytes());
    code.push(0x00);

    // jne <rel8> (patch later)
    code.push(0x75);
    let jne_rel8_pos = code.len();
    code.push(0x00);
    let jne_end = code.len();

    code.push(0x9C); // pushfd
    code.push(0x60); // pushad

    // push target, index, kind (right-to-left for stdcall)
    code.push(0x68);
    code.extend(target_addr.to_le_bytes());
    code.push(0x68);
    code.extend(index.to_le_bytes());
    code.push(0x68);
    code.extend(kind.to_le_bytes());

    // call rel32 (patch later)
    code.push(0xE8);
    let call_rel32_pos = code.len();
    code.extend([0u8; 4]);
    let call_next_ip = code.len();

    code.push(0x61); // popad
    code.push(0x9D); // popfd

    // mov byte ptr [flag_addr], 1
    code.extend([0xC6, 0x05]);
    code.extend(flag_addr.to_le_bytes());
    code.push(0x01);

    let tailjmp_start = code.len();
    code.push(0xB8); // mov eax, imm32
    code.extend(target_addr.to_le_bytes());
    code.extend([0xFF, 0xE0]); // jmp eax

    let jne_rel = (tailjmp_start as isize) - (jne_end as isize);
    if !(-128..=127).contains(&jne_rel) {
        return None;
    }
    code[jne_rel8_pos] = (jne_rel as i8) as u8;

    let stub = unsafe {
        VirtualAlloc(
            std::ptr::null_mut(),
            code.len(),
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
    } as *mut u8;
    if stub.is_null() {
        return None;
    }

    let rel32 = (logger_addr as i64) - ((stub as i64) + (call_next_ip as i64));
    if rel32 < i32::MIN as i64 || rel32 > i32::MAX as i64 {
        return None;
    }
    let rel32_bytes = (rel32 as i32).to_le_bytes();
    code[call_rel32_pos..call_rel32_pos + 4].copy_from_slice(&rel32_bytes);

    unsafe {
        std::ptr::copy_nonoverlapping(code.as_ptr(), stub, code.len());
    }

    Some(stub as *mut c_void)
}

#[cfg(target_pointer_width = "32")]
fn try_patch_com_vtable_for_usage(
    instance: *mut c_void,
    kind: u32,
    max_slots: usize,
    patched_set: &OnceLock<Mutex<HashSet<usize>>>,
) {
    if DIRECTDRAW_VTABLE_USAGE_REPORTED.load(Ordering::Relaxed) != 0 {
        return;
    }
    if instance.is_null() {
        return;
    }

    let instance_addr = instance as usize;
    let set = patched_set.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(guard) = set.lock() {
        if guard.contains(&instance_addr) {
            return;
        }
    } else {
        return;
    }

    // Read the original vtable pointer.
    if !is_readable_ptr(instance, std::mem::size_of::<*const c_void>()) {
        return;
    }
    let orig_vtable = unsafe { *(instance as *const *const *const c_void) };
    if orig_vtable.is_null() {
        return;
    }
    let bytes = max_slots * std::mem::size_of::<*const c_void>();
    if !is_readable_ptr(orig_vtable as *const c_void, bytes) {
        return;
    }

    // Allocate a writable copy of the vtable, and patch entries to tiny stubs.
    let new_vtable = unsafe {
        VirtualAlloc(
            std::ptr::null_mut(),
            bytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    } as *mut *mut c_void;
    if new_vtable.is_null() {
        return;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(orig_vtable as *const *mut c_void, new_vtable, max_slots);
    }

    for i in 0..max_slots {
        let method = unsafe { *orig_vtable.add(i) };
        if method.is_null() {
            continue;
        }
        if !is_ptr_in_directdraw_runtime(method) || !is_executable_ptr(method) {
            continue;
        }
        let Some(stub) = alloc_x86_ddraw_usage_stub(kind, i as u32, method) else {
            continue;
        };
        unsafe {
            *new_vtable.add(i) = stub;
        }
    }

    // Point this interface instance at the patched vtable copy.
    unsafe {
        *(instance as *mut *mut c_void) = new_vtable as *mut c_void;
    }

    if let Ok(mut guard) = set.lock() {
        let _ = guard.insert(instance_addr);
    }
}

fn try_install_directdraw_object_hooks(instance: *mut c_void, source: &str) {
    try_install_directdraw_query_interface_hook(instance, source);
    try_install_directdraw_create_surface_hook(instance, source);
    try_install_directdraw_restore_display_mode_hook(instance, source);
    try_install_directdraw_set_cooperative_level_hook(instance, source);
    if source_is_directdraw_legacy_interface(source) {
        try_install_directdraw_set_display_mode_hook(instance, source);
    }
    if source_is_directdraw_extended_interface(source) {
        try_install_directdraw_set_display_mode_ex_hook(instance, source);
    }
    try_install_directdraw_wait_for_vblank_hook(instance, source);

    #[cfg(target_pointer_width = "32")]
    try_patch_com_vtable_for_usage(instance, 1, 40, &DIRECTDRAW_PATCHED_INTERFACES);
}

fn try_install_directdraw_surface_hooks(surface: *mut c_void, source: &str) {
    try_install_directdraw_surface_blt_hook(surface, source);
    try_install_directdraw_surface_bltfast_hook(surface, source);
    try_install_directdraw_surface_flip_hook(surface, source);
    try_install_directdraw_surface_getattached_hook(surface, source);
    try_install_directdraw_surface_getdc_hook(surface, source);
    try_install_directdraw_surface_getdesc_hook(surface, source);
    try_install_directdraw_surface_islost_hook(surface, source);
    try_install_directdraw_surface_lock_hook(surface, source);
    try_install_directdraw_surface_unlock_hook(surface, source);
    try_install_directdraw_surface_releasedc_hook(surface, source);
    try_install_directdraw_surface_restore_hook(surface, source);
    try_install_directdraw_surface_setclipper_hook(surface, source);
    try_install_directdraw_surface_setpalette_hook(surface, source);

    #[cfg(target_pointer_width = "32")]
    try_patch_com_vtable_for_usage(surface, 2, 64, &DIRECTDRAWSURFACE_PATCHED_INTERFACES);
}

fn ptr_to_fn<T>(ptr: *const c_void) -> T {
    unsafe { std::mem::transmute_copy(&ptr) }
}

fn try_install_directdraw_create_surface_hook(instance: *mut c_void, source: &str) {
    if instance.is_null() {
        return;
    }

    let Some(method_ptr) = vtable_method_ptr(instance, 6) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_CREATE_SURFACE_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_CREATE_SURFACE_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);

    let target_fn: FnDdCreateSurface = ptr_to_fn(method_ptr);
    let use_alt_slot = DD_CREATE_SURFACE_HOOK.get().is_some();
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_create_surface_alt_detour
            } else {
                dd_create_surface_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=CreateSurface ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_CREATE_SURFACE_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_CREATE_SURFACE_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_CREATE_SURFACE_ALT_TARGET.set(method_addr)
        } else {
            DD_CREATE_SURFACE_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_CREATE_SURFACE_ALT_HOOK.get()
        } else {
            DD_CREATE_SURFACE_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => {
                    send_event(make_event(
                        "DirectDrawHookInstall",
                        format!(
                            "source={source} method=CreateSurface ptr={method_ptr:p} in_runtime={in_runtime}"
                        ),
                        "ENABLED".to_owned(),
                    ));
                }
                Err(error) => {
                    send_event(make_event(
                        "DirectDrawHookInstall",
                        format!(
                            "source={source} method=CreateSurface ptr={method_ptr:p} in_runtime={in_runtime}"
                        ),
                        format!("ENABLE_FAILED: {error}"),
                    ));
                }
            }
        }
    } else if use_alt_slot {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=CreateSurface ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
    }
}

fn try_install_directdraw_set_cooperative_level_hook(instance: *mut c_void, source: &str) {
    if instance.is_null() {
        return;
    }

    let Some(method_ptr) = vtable_method_ptr(instance, DD_METHOD_SET_COOPERATIVE_LEVEL_INDEX)
    else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SET_COOPERATIVE_LEVEL_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SET_COOPERATIVE_LEVEL_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);

    let target_fn: FnDdSetCooperativeLevel = ptr_to_fn(method_ptr);
    let use_alt_slot = DD_SET_COOPERATIVE_LEVEL_HOOK.get().is_some();
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_set_cooperative_level_alt_detour
            } else {
                dd_set_cooperative_level_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SetCooperativeLevel ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SET_COOPERATIVE_LEVEL_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SET_COOPERATIVE_LEVEL_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SET_COOPERATIVE_LEVEL_ALT_TARGET.set(method_addr)
        } else {
            DD_SET_COOPERATIVE_LEVEL_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SET_COOPERATIVE_LEVEL_ALT_HOOK.get()
        } else {
            DD_SET_COOPERATIVE_LEVEL_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => {
                    send_event(make_event(
                        "DirectDrawHookInstall",
                        format!(
                            "source={source} method=SetCooperativeLevel ptr={method_ptr:p} in_runtime={in_runtime}"
                        ),
                        "ENABLED".to_owned(),
                    ));
                }
                Err(error) => {
                    send_event(make_event(
                        "DirectDrawHookInstall",
                        format!(
                            "source={source} method=SetCooperativeLevel ptr={method_ptr:p} in_runtime={in_runtime}"
                        ),
                        format!("ENABLE_FAILED: {error}"),
                    ));
                }
            }
        }
    } else if use_alt_slot {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SetCooperativeLevel ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
    }
}

fn try_install_directdraw_set_display_mode_hook(instance: *mut c_void, source: &str) {
    if instance.is_null() {
        return;
    }

    let Some(method_ptr) = vtable_method_ptr(instance, DD_METHOD_SET_DISPLAY_MODE_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SET_DISPLAY_MODE_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SET_DISPLAY_MODE_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SET_DISPLAY_MODE_HOOK.get().is_some();
    if use_alt_slot && DD_SET_DISPLAY_MODE_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SetDisplayMode ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSetDisplayMode = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_set_display_mode_alt_detour
            } else {
                dd_set_display_mode_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SetDisplayMode ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SET_DISPLAY_MODE_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SET_DISPLAY_MODE_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SET_DISPLAY_MODE_ALT_TARGET.set(method_addr)
        } else {
            DD_SET_DISPLAY_MODE_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SET_DISPLAY_MODE_ALT_HOOK.get()
        } else {
            DD_SET_DISPLAY_MODE_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SetDisplayMode ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SetDisplayMode ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_restore_display_mode_hook(instance: *mut c_void, source: &str) {
    if instance.is_null() {
        return;
    }

    let Some(method_ptr) = vtable_method_ptr(instance, DD_METHOD_RESTORE_DISPLAY_MODE_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_RESTORE_DISPLAY_MODE_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_RESTORE_DISPLAY_MODE_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_RESTORE_DISPLAY_MODE_HOOK.get().is_some();
    if use_alt_slot && DD_RESTORE_DISPLAY_MODE_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=RestoreDisplayMode ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdRestoreDisplayMode = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_restore_display_mode_alt_detour
            } else {
                dd_restore_display_mode_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=RestoreDisplayMode ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_RESTORE_DISPLAY_MODE_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_RESTORE_DISPLAY_MODE_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_RESTORE_DISPLAY_MODE_ALT_TARGET.set(method_addr)
        } else {
            DD_RESTORE_DISPLAY_MODE_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_RESTORE_DISPLAY_MODE_ALT_HOOK.get()
        } else {
            DD_RESTORE_DISPLAY_MODE_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=RestoreDisplayMode ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=RestoreDisplayMode ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_set_display_mode_ex_hook(instance: *mut c_void, source: &str) {
    if instance.is_null() {
        return;
    }

    let Some(method_ptr) = vtable_method_ptr(instance, DD_METHOD_SET_DISPLAY_MODE_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SET_DISPLAY_MODE_EX_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SET_DISPLAY_MODE_EX_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SET_DISPLAY_MODE_EX_HOOK.get().is_some();
    if use_alt_slot && DD_SET_DISPLAY_MODE_EX_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SetDisplayModeEx ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSetDisplayModeEx = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_set_display_mode_ex_alt_detour
            } else {
                dd_set_display_mode_ex_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SetDisplayModeEx ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SET_DISPLAY_MODE_EX_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SET_DISPLAY_MODE_EX_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SET_DISPLAY_MODE_EX_ALT_TARGET.set(method_addr)
        } else {
            DD_SET_DISPLAY_MODE_EX_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SET_DISPLAY_MODE_EX_ALT_HOOK.get()
        } else {
            DD_SET_DISPLAY_MODE_EX_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SetDisplayModeEx ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SetDisplayModeEx ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_wait_for_vblank_hook(instance: *mut c_void, source: &str) {
    if instance.is_null() {
        return;
    }

    let Some(method_ptr) = vtable_method_ptr(instance, DD_METHOD_WAIT_FOR_VERTICAL_BLANK_INDEX)
    else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_WAIT_FOR_VBLANK_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_WAIT_FOR_VBLANK_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_WAIT_FOR_VBLANK_HOOK.get().is_some();
    if use_alt_slot && DD_WAIT_FOR_VBLANK_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=WaitForVerticalBlank ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdWaitForVerticalBlank = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_wait_for_vblank_alt_detour
            } else {
                dd_wait_for_vblank_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=WaitForVerticalBlank ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_WAIT_FOR_VBLANK_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_WAIT_FOR_VBLANK_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_WAIT_FOR_VBLANK_ALT_TARGET.set(method_addr)
        } else {
            DD_WAIT_FOR_VBLANK_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_WAIT_FOR_VBLANK_ALT_HOOK.get()
        } else {
            DD_WAIT_FOR_VBLANK_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=WaitForVerticalBlank ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=WaitForVerticalBlank ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_surface_blt_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_BLT_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_BLT_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_BLT_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_BLT_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_BLT_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!("source={source} method=SurfaceBlt ptr={method_ptr:p} in_runtime={in_runtime}"),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceBlt = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_blt_alt_detour
            } else {
                dd_surface_blt_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceBlt ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_BLT_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_BLT_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_BLT_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_BLT_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_BLT_ALT_HOOK.get()
        } else {
            DD_SURFACE_BLT_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceBlt ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceBlt ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_surface_bltfast_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_BLTFAST_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_BLTFAST_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_BLTFAST_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_BLTFAST_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_BLTFAST_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceBltFast ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceBltFast = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_bltfast_alt_detour
            } else {
                dd_surface_bltfast_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceBltFast ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_BLTFAST_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_BLTFAST_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_BLTFAST_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_BLTFAST_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_BLTFAST_ALT_HOOK.get()
        } else {
            DD_SURFACE_BLTFAST_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceBltFast ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceBltFast ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_surface_flip_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_FLIP_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_FLIP_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_FLIP_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_FLIP_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_FLIP_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceFlip ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceFlip = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_flip_alt_detour
            } else {
                dd_surface_flip_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceFlip ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_FLIP_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_FLIP_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_FLIP_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_FLIP_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_FLIP_ALT_HOOK.get()
        } else {
            DD_SURFACE_FLIP_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceFlip ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceFlip ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_surface_getdc_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_GETDC_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_GETDC_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_GETDC_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_GETDC_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_GETDC_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceGetDC ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceGetDC = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_getdc_alt_detour
            } else {
                dd_surface_getdc_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceGetDC ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_GETDC_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_GETDC_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_GETDC_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_GETDC_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_GETDC_ALT_HOOK.get()
        } else {
            DD_SURFACE_GETDC_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceGetDC ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceGetDC ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_surface_lock_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_LOCK_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_LOCK_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_LOCK_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_LOCK_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_LOCK_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceLock ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceLock = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_lock_alt_detour
            } else {
                dd_surface_lock_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceLock ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_LOCK_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_LOCK_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_LOCK_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_LOCK_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_LOCK_ALT_HOOK.get()
        } else {
            DD_SURFACE_LOCK_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceLock ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceLock ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_surface_unlock_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_UNLOCK_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_UNLOCK_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_UNLOCK_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_UNLOCK_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_UNLOCK_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceUnlock ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceUnlock = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_unlock_alt_detour
            } else {
                dd_surface_unlock_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceUnlock ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_UNLOCK_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_UNLOCK_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_UNLOCK_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_UNLOCK_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_UNLOCK_ALT_HOOK.get()
        } else {
            DD_SURFACE_UNLOCK_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceUnlock ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceUnlock ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_surface_releasedc_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_RELEASEDC_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_RELEASEDC_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_RELEASEDC_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_RELEASEDC_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_RELEASEDC_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceReleaseDC ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceReleaseDC = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_releasedc_alt_detour
            } else {
                dd_surface_releasedc_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceReleaseDC ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_RELEASEDC_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_RELEASEDC_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_RELEASEDC_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_RELEASEDC_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_RELEASEDC_ALT_HOOK.get()
        } else {
            DD_SURFACE_RELEASEDC_HOOK.get()
        };
        if let Some(h) = hook_ref {
            match unsafe { h.enable() } {
                Ok(()) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceReleaseDC ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    "ENABLED".to_owned(),
                )),
                Err(error) => send_event(make_event(
                    "DirectDrawHookInstall",
                    format!(
                        "source={source} method=SurfaceReleaseDC ptr={method_ptr:p} in_runtime={in_runtime}"
                    ),
                    format!("ENABLE_FAILED: {error}"),
                )),
            }
        }
    }
}

fn try_install_directdraw_surface_islost_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_ISLOST_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_ISLOST_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_ISLOST_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_ISLOST_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_ISLOST_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceIsLost ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceIsLost = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_islost_alt_detour
            } else {
                dd_surface_islost_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceIsLost ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_ISLOST_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_ISLOST_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_ISLOST_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_ISLOST_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_ISLOST_ALT_HOOK.get()
        } else {
            DD_SURFACE_ISLOST_HOOK.get()
        };
        if let Some(h) = hook_ref {
            let _ = unsafe { h.enable() };
        }
    }
}

fn try_install_directdraw_surface_restore_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_RESTORE_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_RESTORE_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_RESTORE_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_RESTORE_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_RESTORE_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceRestore ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceRestore = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_restore_alt_detour
            } else {
                dd_surface_restore_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceRestore ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_RESTORE_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_RESTORE_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_RESTORE_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_RESTORE_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_RESTORE_ALT_HOOK.get()
        } else {
            DD_SURFACE_RESTORE_HOOK.get()
        };
        if let Some(h) = hook_ref {
            let _ = unsafe { h.enable() };
        }
    }
}

fn try_install_directdraw_surface_getdesc_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_GETDESC_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_GETDESC_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_GETDESC_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_GETDESC_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_GETDESC_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceGetSurfaceDesc ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceGetSurfaceDesc = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_getdesc_alt_detour
            } else {
                dd_surface_getdesc_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceGetSurfaceDesc ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_GETDESC_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_GETDESC_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_GETDESC_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_GETDESC_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_GETDESC_ALT_HOOK.get()
        } else {
            DD_SURFACE_GETDESC_HOOK.get()
        };
        if let Some(h) = hook_ref {
            let _ = unsafe { h.enable() };
        }
    }
}

fn try_install_directdraw_surface_getattached_hook(surface: *mut c_void, source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_GETATTACHED_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_GETATTACHED_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_GETATTACHED_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_GETATTACHED_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_GETATTACHED_ALT_HOOK.get().is_some() {
        send_event(make_event(
            "DirectDrawHookInstall",
            format!(
                "source={source} method=SurfaceGetAttachedSurface ptr={method_ptr:p} in_runtime={in_runtime}"
            ),
            "SKIPPED: alt slot already in use".to_owned(),
        ));
        return;
    }
    let target_fn: FnDdSurfaceGetAttachedSurface = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_getattached_alt_detour
            } else {
                dd_surface_getattached_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=SurfaceGetAttachedSurface ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_GETATTACHED_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_GETATTACHED_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_GETATTACHED_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_GETATTACHED_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_GETATTACHED_ALT_HOOK.get()
        } else {
            DD_SURFACE_GETATTACHED_HOOK.get()
        };
        if let Some(h) = hook_ref {
            let _ = unsafe { h.enable() };
        }
    }
}

fn try_install_directdraw_surface_setclipper_hook(surface: *mut c_void, _source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_SETCLIPPER_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_SETCLIPPER_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_SETCLIPPER_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let _in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_SETCLIPPER_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_SETCLIPPER_ALT_HOOK.get().is_some() {
        return;
    }
    let target_fn: FnDdSurfaceSetClipper = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_setclipper_alt_detour
            } else {
                dd_surface_setclipper_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(_) => return,
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_SETCLIPPER_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_SETCLIPPER_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_SETCLIPPER_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_SETCLIPPER_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_SETCLIPPER_ALT_HOOK.get()
        } else {
            DD_SURFACE_SETCLIPPER_HOOK.get()
        };
        if let Some(h) = hook_ref {
            let _ = unsafe { h.enable() };
        }
    }
}

fn try_install_directdraw_surface_setpalette_hook(surface: *mut c_void, _source: &str) {
    if surface.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(surface, DDS_METHOD_SETPALETTE_INDEX) else {
        return;
    };
    let method_addr = method_ptr as usize;
    if DD_SURFACE_SETPALETTE_TARGET
        .get()
        .is_some_and(|addr| *addr == method_addr)
        || DD_SURFACE_SETPALETTE_ALT_TARGET
            .get()
            .is_some_and(|addr| *addr == method_addr)
    {
        return;
    }
    let _in_runtime = is_ptr_in_directdraw_runtime(method_ptr);
    let use_alt_slot = DD_SURFACE_SETPALETTE_HOOK.get().is_some();
    if use_alt_slot && DD_SURFACE_SETPALETTE_ALT_HOOK.get().is_some() {
        return;
    }
    let target_fn: FnDdSurfaceSetPalette = ptr_to_fn(method_ptr);
    let hook = match unsafe {
        GenericDetour::new(
            target_fn,
            if use_alt_slot {
                dd_surface_setpalette_alt_detour
            } else {
                dd_surface_setpalette_detour
            },
        )
    } {
        Ok(hook) => hook,
        Err(_) => return,
    };

    let install_ok = if use_alt_slot {
        DD_SURFACE_SETPALETTE_ALT_HOOK.set(hook).is_ok()
    } else {
        DD_SURFACE_SETPALETTE_HOOK.set(hook).is_ok()
    };
    if install_ok {
        let _ = if use_alt_slot {
            DD_SURFACE_SETPALETTE_ALT_TARGET.set(method_addr)
        } else {
            DD_SURFACE_SETPALETTE_TARGET.set(method_addr)
        };
        let hook_ref = if use_alt_slot {
            DD_SURFACE_SETPALETTE_ALT_HOOK.get()
        } else {
            DD_SURFACE_SETPALETTE_HOOK.get()
        };
        if let Some(h) = hook_ref {
            let _ = unsafe { h.enable() };
        }
    }
}

fn call_directdraw_query_interface(instance: *mut c_void, iid: &Guid) -> Option<*mut c_void> {
    if instance.is_null() {
        return None;
    }
    let Some(method_ptr) = vtable_method_ptr(instance, 0) else {
        return None;
    };

    let query_fn: FnDdQueryInterface = ptr_to_fn(method_ptr);
    let mut out: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        query_fn(
            instance,
            iid as *const Guid as *const c_void,
            &mut out as *mut *mut c_void,
        )
    };
    if hresult_succeeded(hr) && !out.is_null() {
        Some(out)
    } else {
        None
    }
}

fn call_directdraw_release(instance: *mut c_void) {
    if instance.is_null() {
        return;
    }
    let Some(method_ptr) = vtable_method_ptr(instance, 2) else {
        return;
    };

    let release_fn: FnDdRelease = ptr_to_fn(method_ptr);
    let _ = unsafe { release_fn(instance) };
}

fn try_probe_directdraw_interfaces(instance: *mut c_void, source: &str) {
    if instance.is_null() {
        return;
    }

    let probes = [
        (&IID_IDIRECTDRAW, "IDirectDraw"),
        (&IID_IDIRECTDRAW2, "IDirectDraw2"),
        (&IID_IDIRECTDRAW4, "IDirectDraw4"),
        (&IID_IDIRECTDRAW7, "IDirectDraw7"),
    ];

    for (iid, name) in probes {
        let Some(interface_ptr) = call_directdraw_query_interface(instance, iid) else {
            continue;
        };
        let hook_source = format!("{source}->{name}");
        try_install_directdraw_object_hooks(interface_ptr, &hook_source);
        call_directdraw_release(interface_ptr);
    }
}

fn try_install_directdraw_query_interface_hook(instance: *mut c_void, source: &str) {
    if DD_QUERY_INTERFACE_HOOK.get().is_some() || instance.is_null() {
        return;
    }

    let Some(method_ptr) = vtable_method_ptr(instance, 0) else {
        return;
    };
    let in_runtime = is_ptr_in_directdraw_runtime(method_ptr);

    let target_fn: FnDdQueryInterface = ptr_to_fn(method_ptr);
    let hook = match unsafe { GenericDetour::new(target_fn, dd_query_interface_detour) } {
        Ok(hook) => hook,
        Err(error) => {
            send_event(make_event(
                "DirectDrawHookInstall",
                format!(
                    "source={source} method=QueryInterface ptr={method_ptr:p} in_runtime={in_runtime}"
                ),
                format!("INIT_FAILED: {error}"),
            ));
            return;
        }
    };

    if DD_QUERY_INTERFACE_HOOK.set(hook).is_ok() {
        if let Some(h) = DD_QUERY_INTERFACE_HOOK.get() {
            match unsafe { h.enable() } {
                Ok(()) => {
                    send_event(make_event(
                        "DirectDrawHookInstall",
                        format!(
                            "source={source} method=QueryInterface ptr={method_ptr:p} in_runtime={in_runtime}"
                        ),
                        "ENABLED".to_owned(),
                    ));
                }
                Err(error) => {
                    send_event(make_event(
                        "DirectDrawHookInstall",
                        format!(
                            "source={source} method=QueryInterface ptr={method_ptr:p} in_runtime={in_runtime}"
                        ),
                        format!("ENABLE_FAILED: {error}"),
                    ));
                }
            }
        }
    }
}

fn elapsed_ms() -> u64 {
    let started_at = START_TIME.get_or_init(Instant::now);
    let millis = started_at.elapsed().as_millis();
    millis.min(u64::MAX as u128) as u64
}

fn try_install_optional_graphics_hooks() -> Result<(), String> {
    let lock = OPTIONAL_HOOK_INSTALL_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock
        .lock()
        .map_err(|_| "optional hook install lock poisoned".to_owned())?;

    install_optional_hook_directdraw_create()?;
    install_optional_hook_directdraw_create_ex()?;
    install_optional_hook_directdraw_create_clipper()?;
    install_optional_hook_directdraw_enumerate_a()?;
    install_optional_hook_directdraw_enumerate_w()?;
    install_optional_hook_directdraw_enumerate_ex_a()?;
    install_optional_hook_directdraw_enumerate_ex_w()?;
    install_optional_hook_co_create_instance()?;
    install_optional_hook_co_create_instance_ex()?;
    install_optional_hook_direct3d_create9()?;
    install_optional_hook_direct3d_create9_ex()?;
    install_optional_hook_create_dxgi_factory()?;
    install_optional_hook_create_dxgi_factory1()?;
    install_optional_hook_d3d11_create_device()?;
    install_optional_hook_d3d11_create_device_and_swap_chain()?;

    Ok(())
}

fn install_optional_hook_directdraw_create() -> Result<(), String> {
    if DIRECTDRAW_CREATE_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnDirectDrawCreate>(
            b"ddraw.dll\0",
            b"DirectDrawCreate\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, directdraw_create_detour) }
        .map_err(|e| format!("DirectDrawCreate late init failed: {e}"))?;
    if DIRECTDRAW_CREATE_HOOK.set(hook).is_ok() {
        if let Some(h) = DIRECTDRAW_CREATE_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("DirectDrawCreate late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_directdraw_create_ex() -> Result<(), String> {
    if DIRECTDRAW_CREATE_EX_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnDirectDrawCreateEx>(
            b"ddraw.dll\0",
            b"DirectDrawCreateEx\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, directdraw_create_ex_detour) }
        .map_err(|e| format!("DirectDrawCreateEx late init failed: {e}"))?;
    if DIRECTDRAW_CREATE_EX_HOOK.set(hook).is_ok() {
        if let Some(h) = DIRECTDRAW_CREATE_EX_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("DirectDrawCreateEx late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_directdraw_create_clipper() -> Result<(), String> {
    if DIRECTDRAW_CREATE_CLIPPER_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnDirectDrawCreateClipper>(
            b"ddraw.dll\0",
            b"DirectDrawCreateClipper\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, directdraw_create_clipper_detour) }
        .map_err(|e| format!("DirectDrawCreateClipper late init failed: {e}"))?;
    if DIRECTDRAW_CREATE_CLIPPER_HOOK.set(hook).is_ok() {
        if let Some(h) = DIRECTDRAW_CREATE_CLIPPER_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("DirectDrawCreateClipper late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_directdraw_enumerate_a() -> Result<(), String> {
    if DIRECTDRAW_ENUMERATE_A_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnDirectDrawEnumerateA>(
            b"ddraw.dll\0",
            b"DirectDrawEnumerateA\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, directdraw_enumerate_a_detour) }
        .map_err(|e| format!("DirectDrawEnumerateA late init failed: {e}"))?;
    if DIRECTDRAW_ENUMERATE_A_HOOK.set(hook).is_ok() {
        if let Some(h) = DIRECTDRAW_ENUMERATE_A_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("DirectDrawEnumerateA late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_directdraw_enumerate_w() -> Result<(), String> {
    if DIRECTDRAW_ENUMERATE_W_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnDirectDrawEnumerateW>(
            b"ddraw.dll\0",
            b"DirectDrawEnumerateW\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, directdraw_enumerate_w_detour) }
        .map_err(|e| format!("DirectDrawEnumerateW late init failed: {e}"))?;
    if DIRECTDRAW_ENUMERATE_W_HOOK.set(hook).is_ok() {
        if let Some(h) = DIRECTDRAW_ENUMERATE_W_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("DirectDrawEnumerateW late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_directdraw_enumerate_ex_a() -> Result<(), String> {
    if DIRECTDRAW_ENUMERATE_EX_A_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnDirectDrawEnumerateExA>(
            b"ddraw.dll\0",
            b"DirectDrawEnumerateExA\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, directdraw_enumerate_ex_a_detour) }
        .map_err(|e| format!("DirectDrawEnumerateExA late init failed: {e}"))?;
    if DIRECTDRAW_ENUMERATE_EX_A_HOOK.set(hook).is_ok() {
        if let Some(h) = DIRECTDRAW_ENUMERATE_EX_A_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("DirectDrawEnumerateExA late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_directdraw_enumerate_ex_w() -> Result<(), String> {
    if DIRECTDRAW_ENUMERATE_EX_W_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnDirectDrawEnumerateExW>(
            b"ddraw.dll\0",
            b"DirectDrawEnumerateExW\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, directdraw_enumerate_ex_w_detour) }
        .map_err(|e| format!("DirectDrawEnumerateExW late init failed: {e}"))?;
    if DIRECTDRAW_ENUMERATE_EX_W_HOOK.set(hook).is_ok() {
        if let Some(h) = DIRECTDRAW_ENUMERATE_EX_W_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("DirectDrawEnumerateExW late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_co_create_instance() -> Result<(), String> {
    if CO_CREATE_INSTANCE_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnCoCreateInstance>(
            b"ole32.dll\0",
            b"CoCreateInstance\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, co_create_instance_detour) }
        .map_err(|e| format!("CoCreateInstance late init failed: {e}"))?;
    if CO_CREATE_INSTANCE_HOOK.set(hook).is_ok() {
        if let Some(h) = CO_CREATE_INSTANCE_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("CoCreateInstance late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_co_create_instance_ex() -> Result<(), String> {
    if CO_CREATE_INSTANCE_EX_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnCoCreateInstanceEx>(
            b"ole32.dll\0",
            b"CoCreateInstanceEx\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, co_create_instance_ex_detour) }
        .map_err(|e| format!("CoCreateInstanceEx late init failed: {e}"))?;
    if CO_CREATE_INSTANCE_EX_HOOK.set(hook).is_ok() {
        if let Some(h) = CO_CREATE_INSTANCE_EX_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("CoCreateInstanceEx late enable failed: {e}"))?;
        }
    }

    Ok(())
}

fn install_optional_hook_direct3d_create9() -> Result<(), String> {
    if DIRECT3D_CREATE9_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnDirect3DCreate9>(b"d3d9.dll\0", b"Direct3DCreate9\0")
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, direct3d_create9_detour) }
        .map_err(|e| format!("Direct3DCreate9 late init failed: {e}"))?;
    if DIRECT3D_CREATE9_HOOK.set(hook).is_ok() {
        if let Some(h) = DIRECT3D_CREATE9_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("Direct3DCreate9 late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_direct3d_create9_ex() -> Result<(), String> {
    if DIRECT3D_CREATE9_EX_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnDirect3DCreate9Ex>(
            b"d3d9.dll\0",
            b"Direct3DCreate9Ex\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, direct3d_create9_ex_detour) }
        .map_err(|e| format!("Direct3DCreate9Ex late init failed: {e}"))?;
    if DIRECT3D_CREATE9_EX_HOOK.set(hook).is_ok() {
        if let Some(h) = DIRECT3D_CREATE9_EX_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("Direct3DCreate9Ex late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_create_dxgi_factory() -> Result<(), String> {
    if CREATE_DXGI_FACTORY_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnCreateDXGIFactory>(
            b"dxgi.dll\0",
            b"CreateDXGIFactory\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, create_dxgi_factory_detour) }
        .map_err(|e| format!("CreateDXGIFactory late init failed: {e}"))?;
    if CREATE_DXGI_FACTORY_HOOK.set(hook).is_ok() {
        if let Some(h) = CREATE_DXGI_FACTORY_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("CreateDXGIFactory late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_create_dxgi_factory1() -> Result<(), String> {
    if CREATE_DXGI_FACTORY1_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnCreateDXGIFactory1>(
            b"dxgi.dll\0",
            b"CreateDXGIFactory1\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, create_dxgi_factory1_detour) }
        .map_err(|e| format!("CreateDXGIFactory1 late init failed: {e}"))?;
    if CREATE_DXGI_FACTORY1_HOOK.set(hook).is_ok() {
        if let Some(h) = CREATE_DXGI_FACTORY1_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("CreateDXGIFactory1 late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_d3d11_create_device() -> Result<(), String> {
    if D3D11_CREATE_DEVICE_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnD3D11CreateDevice>(
            b"d3d11.dll\0",
            b"D3D11CreateDevice\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, d3d11_create_device_detour) }
        .map_err(|e| format!("D3D11CreateDevice late init failed: {e}"))?;
    if D3D11_CREATE_DEVICE_HOOK.set(hook).is_ok() {
        if let Some(h) = D3D11_CREATE_DEVICE_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("D3D11CreateDevice late enable failed: {e}"))?;
        }
    }
    Ok(())
}

fn install_optional_hook_d3d11_create_device_and_swap_chain() -> Result<(), String> {
    if D3D11_CREATE_DEVICE_AND_SWAP_CHAIN_HOOK.get().is_some() {
        return Ok(());
    }

    let Some(target) = (unsafe {
        try_resolve_proc_in_loaded_module::<FnD3D11CreateDeviceAndSwapChain>(
            b"d3d11.dll\0",
            b"D3D11CreateDeviceAndSwapChain\0",
        )
    }) else {
        return Ok(());
    };

    let hook = unsafe { GenericDetour::new(target, d3d11_create_device_and_swap_chain_detour) }
        .map_err(|e| format!("D3D11CreateDeviceAndSwapChain late init failed: {e}"))?;
    if D3D11_CREATE_DEVICE_AND_SWAP_CHAIN_HOOK.set(hook).is_ok() {
        if let Some(h) = D3D11_CREATE_DEVICE_AND_SWAP_CHAIN_HOOK.get() {
            unsafe { h.enable() }
                .map_err(|e| format!("D3D11CreateDeviceAndSwapChain late enable failed: {e}"))?;
        }
    }
    Ok(())
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
) -> Option<T> {
    let module = unsafe { LoadLibraryA(module_name.as_ptr()) };
    if module.is_null() {
        return None;
    }

    let proc = unsafe { GetProcAddress(module, proc_name.as_ptr()) };
    proc.map(proc_to_fn)
}

unsafe fn try_resolve_proc_in_loaded_module<T>(
    module_name: &'static [u8],
    proc_name: &'static [u8],
) -> Option<T> {
    let module = unsafe { GetModuleHandleA(module_name.as_ptr()) };
    if module.is_null() {
        return None;
    }

    let proc = unsafe { GetProcAddress(module, proc_name.as_ptr()) };
    proc.map(proc_to_fn)
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
