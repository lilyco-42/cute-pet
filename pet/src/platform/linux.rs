//! Linux (X11) 桌宠窗口平台层: 置顶 + 无边框 + 整窗透明度。
//!
//! ⚠️ 未在真机验证 — 需 Linux 实机测试。
//! - 置顶: `_NET_WM_STATE_ABOVE`
//! - 无边框: `_MOTIF_WM_HINTS`
//! - 整窗透明度: `_NET_WM_WINDOW_OPACITY`(可选)
//! - ⚠️ 逐像素透明需要 ARGB 视觉(当前未启用, 透明区域会显示为黑底) —
//!   完整逐像素方案: miniquad 用 ARGB visual + 合成器, 属后续工作。
//! - Wayland: 需 layer-shell 协议扩展, 另案处理。
#![cfg(all(target_os = "linux", not(target_env = "ohos")))]

use std::os::raw::{c_char, c_int, c_ulong, c_void};

type Display = c_void;
type Window = c_ulong;
type Atom = c_ulong;

#[link(name = "X11")]
extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut Display;
    fn XDefaultRootWindow(dpy: *mut Display) -> Window;
    fn XQueryTree(
        dpy: *mut Display,
        w: Window,
        root_return: *mut Window,
        parent_return: *mut Window,
        children_return: *mut *mut Window,
        nchildren_return: *mut u32,
    ) -> c_int;
    fn XFetchName(dpy: *mut Display, w: Window) -> *mut c_char;
    fn XFree(data: *mut c_void);
    fn XInternAtom(dpy: *mut Display, name: *const c_char, only_if_exists: c_int) -> Atom;
    fn XChangeProperty(
        dpy: *mut Display,
        w: Window,
        property: Atom,
        type_: Atom,
        format: c_int,
        mode: c_int,
        data: *const c_void,
        nelements: c_int,
    ) -> c_int;
    fn XFlush(dpy: *mut Display);
    fn XCloseDisplay(dpy: *mut Display);
}

const PROP_MODE_REPLACE: c_int = 0;
const XA_CARDINAL: Atom = 6;
const XA_ATOM: Atom = 4;
const MOTIF_HINTS: &[u8] = b"_MOTIF_WM_HINTS\0";
const NET_WM_STATE: &[u8] = b"_NET_WM_STATE\0";
const NET_WM_STATE_ABOVE: &[u8] = b"_NET_WM_STATE_ABOVE\0";
const NET_WM_WINDOW_OPACITY: &[u8] = b"_NET_WM_WINDOW_OPACITY\0";

fn atom(dpy: *mut Display, name: &[u8]) -> Atom {
    let c = std::ffi::CString::new(name).unwrap();
    unsafe { XInternAtom(dpy, c.as_ptr(), 0) }
}

fn find_window_by_title(dpy: *mut Display, title: &str, w: Window, depth: u32) -> Option<Window> {
    if depth > 6 {
        return None;
    }
    unsafe {
        let name = XFetchName(dpy, w);
        if !name.is_null() {
            let s = std::ffi::CStr::from_ptr(name).to_string_lossy();
            let matched = s == title;
            XFree(name as *mut c_void);
            if matched {
                return Some(w);
            }
        }
        let mut root = 0;
        let mut parent = 0;
        let mut children: *mut Window = std::ptr::null_mut();
        let mut n = 0u32;
        if XQueryTree(dpy, w, &mut root, &mut parent, &mut children, &mut n) != 0 {
            let kids = std::slice::from_raw_parts(children, n as usize);
            for &kid in kids {
                if let Some(found) = find_window_by_title(dpy, title, kid, depth + 1) {
                    XFree(children as *mut c_void);
                    return Some(found);
                }
            }
            if !children.is_null() {
                XFree(children as *mut c_void);
            }
        }
    }
    None
}

/// 把窗口改造成置顶无边框桌宠窗口。`title` 需匹配 window_conf 的 window_title。
pub fn make_transparent_pet_window(title: &str) {
    let dpy_name = std::ffi::CString::new("").unwrap();
    unsafe {
        let dpy = XOpenDisplay(dpy_name.as_ptr());
        if dpy.is_null() {
            eprintln!("[linux] XOpenDisplay 失败");
            return;
        }
        let root = XDefaultRootWindow(dpy);
        let win = find_window_by_title(dpy, title, root, 0);
        let win = match win {
            Some(w) => w,
            None => {
                eprintln!("[linux] 未找到窗口 '{title}'");
                XCloseDisplay(dpy);
                return;
            }
        };

        // 置顶: _NET_WM_STATE_ABOVE
        let state_atom = atom(dpy, NET_WM_STATE);
        let above_atom = atom(dpy, NET_WM_STATE_ABOVE);
        XChangeProperty(dpy, win, state_atom, XA_ATOM, 32, PROP_MODE_REPLACE,
            &above_atom as *const Atom as *const c_void, 1);

        // 无边框: _MOTIF_WM_HINTS (flags=1<<1 DECORATIONS, decorations=0)
        let mut hints = [0u64; 5];
        hints[0] = 1 << 1; // MWM_HINTS_DECORATIONS
        hints[2] = 0; // decorations off
        let mh = atom(dpy, MOTIF_HINTS);
        XChangeProperty(dpy, win, mh, mh, 32, PROP_MODE_REPLACE,
            hints.as_ptr() as *const c_void, 5);

        // 可选: 整窗透明度 90% (0xE6000000)
        let opacity = 0u64 | 0xE6000000;
        let oa = atom(dpy, NET_WM_WINDOW_OPACITY);
        XChangeProperty(dpy, win, oa, XA_CARDINAL, 32, PROP_MODE_REPLACE,
            &opacity as *const u64 as *const c_void, 1);

        XFlush(dpy);
        XCloseDisplay(dpy);
        println!("[linux] 置顶无边框窗口已设置 (win={})", win);
    }
}
