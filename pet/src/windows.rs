//! Windows 桌宠窗口平台层: 无边框 + 分层透明(逐像素 alpha) + 置顶。
//! miniquad-ply 原生不支持, 这里拿到 HWND 后应用 Win32 样式 + DWM 逐像素透明。
#![cfg(target_os = "windows")]

use std::os::raw::c_void;

const GWL_STYLE: i32 = -16;
const GWL_EXSTYLE: i32 = -20;

const WS_POPUP: isize = 0x8000_0000;
const WS_EX_LAYERED: isize = 0x0008_0000;
const WS_EX_TOPMOST: isize = 0x0000_0008;
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;

const SWP_FRAMECHANGED: u32 = 0x0020;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;

#[repr(C)]
#[derive(Clone, Copy)]
struct Margins {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    fn SetWindowLongPtrW(hwnd: *mut c_void, n_index: i32, dw_new_long: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: *mut c_void, n_index: i32) -> isize;
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

#[link(name = "dwmapi")]
extern "system" {
    fn DwmExtendFrameIntoClientArea(hwnd: *mut c_void, margins: *const Margins) -> i32;
}

/// 把 miniquad 的窗口改造成透明置顶桌宠窗口。只做一次(启动时)。
pub fn make_transparent_pet_window(hwnd: *mut c_void) {
    if hwnd.is_null() {
        eprintln!("[window] 获取 HWND 失败");
        return;
    }
    unsafe {
        // 1) 无边框: WS_POPUP(去掉标题栏/边框)
        SetWindowLongPtrW(hwnd, GWL_STYLE, WS_POPUP);

        // 2) 分层 + 置顶 + 工具窗(不进任务栏)
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW);

        // 3) DWM 逐像素透明: 负边距让整个客户区参与 DWM 合成, framebuffer alpha → 窗口透明度
        let margins = Margins { left: -1, right: -1, top: -1, bottom: -1 };
        DwmExtendFrameIntoClientArea(hwnd, &margins);

        // 4) 应用样式变更
        SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);

        println!("[window] 透明置顶窗口就绪 (hwnd={:?})", hwnd);
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
