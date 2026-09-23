//! Windows 桌宠窗口平台层: 无边框 + 色键透明(COLORKEY 洋红) + 置顶 + 底部居中。
//!
//! 方案(参考 FocuserPetProject / iwakura-lain/desktop-pet): `WS_EX_LAYERED` +
//! `SetLayeredWindowAttributes(LWA_COLORKEY)` 抠掉纯洋红背景。色键是逐像素
//! 色匹配, 不依赖 DWM 对 OpenGL alpha 通道的处理, 在 AMD 驱动下可靠。
//!
//! 注意: 立绘/UI 不能使用纯洋红(255,0,255)作为前景色。
//! WS_POPUP 已确认会破坏 miniquad 的 OpenGL 呈现(SetPixelFormat 后改样式),
//! 故不设; SetWindowPos 改尺寸也会破坏呈现, 故**尺寸由宏 quad set_window_size
//! 控制**, 这里只做透明 + 置顶 + 位置(不传 cx/cy, 用 SWP_NOSIZE)。
#![cfg(target_os = "windows")]

use std::os::raw::c_void;

const GWL_EXSTYLE: i32 = -20;

const WS_EX_LAYERED: isize = 0x0008_0000;
const WS_EX_TOPMOST: isize = 0x0000_0008;
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;

const SWP_FRAMECHANGED: u32 = 0x0020;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;

/// 色键: 纯洋红(与 main.rs 桌面清屏色一致)。COLORREF = 0x00BBGGRR。
const COLOR_KEY: u32 = 0x00FF_00FF;
/// SetLayeredWindowAttributes flags: 用 crKey 作为透明色。
const LWA_COLORKEY: u32 = 0x0000_0001;

#[link(name = "user32")]
extern "system" {
    fn GetWindowLongPtrW(hwnd: *mut c_void, n_index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: *mut c_void, n_index: i32, dw_new_long: isize) -> isize;
    fn SetLayeredWindowAttributes(hwnd: *mut c_void, cr_key: u32, b_alpha: u8, dw_flags: u32) -> i32;
    fn GetSystemMetrics(n_index: i32) -> i32;
    fn SetWindowPos(
        hwnd: *mut c_void,
        insert_after: *mut c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

/// 把 miniquad 的窗口改造成透明置顶桌宠窗口。
/// 只做透明 + 置顶 + 底部居中定位, **不动尺寸**(改尺寸会破坏 OpenGL 呈现,
/// 尺寸由 main.rs 的宏 quad set_window_size 控制)。
pub fn make_transparent_pet_window(hwnd: *mut c_void) {
    if hwnd.is_null() {
        eprintln!("[window] 获取 HWND 失败");
        return;
    }
    unsafe {
        // 置顶 + 工具窗 + 分层色键。不设 WS_POPUP(破坏 OpenGL 呈现)。
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW);

        // 色键透明: 洋红像素 → 完全透明(仅此色)
        SetLayeredWindowAttributes(hwnd, COLOR_KEY, 0, LWA_COLORKEY);

        // 底部居中定位(仅移动, 不传 cx/cy → SWP_NOSIZE 保留当前尺寸)。
        let sw = GetSystemMetrics(0); // SM_CXSCREEN
        let sh = GetSystemMetrics(1); // SM_CYSCREEN
        SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED | SWP_NOZORDER | SWP_NOACTIVATE);

        println!("[window] 透明置顶桌宠就绪 hwnd={hwnd:?} 屏幕 {sw}x{sh}");
    }
}

/// 移动窗口到屏幕坐标(x, y)。
pub fn move_window(hwnd: *mut c_void, x: i32, y: i32) {
    if hwnd.is_null() {
        return;
    }
    unsafe {
        SetWindowPos(hwnd, std::ptr::null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

/// 底部居中定位桌宠窗口(不改尺寸, 尺寸由宏 quad/conf 控制, 避免呈现冲突)。
pub fn position_bottom_center(hwnd: *mut c_void) {
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let sw = GetSystemMetrics(0); // SM_CXSCREEN
        let sh = GetSystemMetrics(1); // SM_CYSCREEN
        // 读当前窗口尺寸, 只移动位置
        #[repr(C)]
        struct R { l: i32, t: i32, r: i32, b: i32 }
        let mut r = R { l: 0, t: 0, r: 0, b: 0 };
        extern "system" { fn GetWindowRect(h: *mut c_void, r: *mut R) -> i32; }
        GetWindowRect(hwnd, &mut r);
        let w = (r.r - r.l) as u32;
        let h = (r.b - r.t) as u32;
        let x = ((sw as u32).saturating_sub(w) / 2) as i32;
        let y = (sh as u32).saturating_sub(h) as i32;
        SetWindowPos(hwnd, std::ptr::null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        println!("[window] 底部居中: ({x}, {y}) 窗口 {w}x{h} 屏幕 {sw}x{sh}");
    }
}