//! macOS 桌宠窗口平台层: 透明 + 无边框 + 置顶。
//!
//! 通过 Objective-C runtime FFI 操作 AppKit NSWindow。
//! ⚠️ 未在真机验证 — 需 macOS 实机测试, selector/签名可能需微调。
//! 原理: NSApp 的主窗口 → setOpaque:NO + 透明背景 + 去标题栏 + 浮动层级。
#![cfg(target_os = "macos")]

use std::os::raw::{c_char, c_void};

const NS_FLOATING_WINDOW_LEVEL: isize = 3; // NSNormalWindowLevel=0, NSFloatingWindowLevel=3

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    // 变长 objc_msgSend: 标量/指针返回走同一返回寄存器, 调用处 cast 即可
    fn objc_msgSend(obj: *mut c_void, sel: *mut c_void, ...) -> *mut c_void;
}

fn sel(name: &str) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap();
    unsafe { sel_registerName(c.as_ptr()) }
}

fn cls(name: &str) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap();
    unsafe { objc_getClass(c.as_ptr()) }
}

/// 把当前应用主窗口改造成透明置顶桌宠窗口。
pub fn make_transparent_pet_window() {
    unsafe {
        // NSApplication.sharedApplication → mainWindow
        let app = objc_msgSend(cls("NSApplication"), sel("sharedApplication"));
        if app.is_null() {
            eprintln!("[macos] 获取 NSApplication 失败");
            return;
        }
        let window = objc_msgSend(app, sel("mainWindow"));
        if window.is_null() {
            eprintln!("[macos] 未找到主窗口");
            return;
        }

        // setOpaque:NO
        objc_msgSend(window, sel("setOpaque:"), 0i32); // NO (BOOL 以 i32 传 vararg)
        // setBackgroundColor:[NSColor clearColor]
        let clear = objc_msgSend(cls("NSColor"), sel("clearColor"));
        objc_msgSend(window, sel("setBackgroundColor:"), clear);
        // 去标题栏: styleMask &= ~Titled (NSUInteger → u64 vararg)
        let mask = objc_msgSend(window, sel("styleMask")) as isize;
        objc_msgSend(window, sel("setStyleMask:"), (mask & !(1isize << 0)) as u64);
        // 浮动层级(置顶): NSInteger → isize vararg
        objc_msgSend(window, sel("setLevel:"), NS_FLOATING_WINDOW_LEVEL as isize);
        // 重新显示以应用样式
        objc_msgSend(window, sel("display"));
        println!("[macos] 透明置顶窗口已设置");
    }
}
