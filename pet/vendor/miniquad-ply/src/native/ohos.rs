//! HarmonyOS (OpenHarmony) native backend for miniquad-ply.
//!
//! 仿 `android.rs` 的结构: 打电话
//! - 一个渲染线程负责 EGL 上下文初始化 + 渲染循环(frame/update)
//! - 主线程(ArkTS 侧)通过 NAPI 把 XComponent 的 surface 事件投递到渲染线程
//!
//! 与 Android 的差异:
//! - 窗口: 鸿蒙用 ArkTS `XComponent` + NAPI `OH_NativeXComponent`(surface → OHNativeWindow),
//!   而非 Android 的 Java Activity + ANativeWindow
//! - 入口: NAPI `napi_module_register` + `nm_register_func`, 面非 JNI_OnLoad
//! - 资产: 应用自身通过 rust-embed 内嵌, 此 backend 不涉及 AAssetManager
//!
//! 状态: WIP — 渲染线程/EGL 完整; XComponent surface 获取依赖宿主 ArkTS 配置本 NAPI。

use crate::{
    event::{EventHandler, KeyCode, KeyMods, TouchPhase},
    native::{
        egl::{self, LibEgl},
        NativeDisplayData,
    },
};

use std::{sync::mpsc, sync::Mutex, thread, time::Duration};

pub use crate::native::gl::{self, *};

/// 渲染线程与主线程之间的消息(仿 android::Message)。
#[derive(Debug)]
enum Message {
    SurfaceChanged { width: i32, height: i32 },
    SurfaceCreated { window: *mut core::ffi::c_void }, // OHNativeWindow*
    SurfaceDestroyed,
    Touch { phase: TouchPhase, touch_id: u64, x: f32, y: f32 },
    Character { character: u32 },
    KeyDown { keycode: KeyCode },
    KeyUp { keycode: KeyCode },
    Pause,
    Resume,
    Destroy,
    Request(crate::native::Request),
}
unsafe impl Send for Message {}

// 跨线程消息发送(JS 线程 + IME 回调线程共用; 渲染线程用 rx 接收)
// 之前用 thread_local! 导致 IME 回调线程 panic(thread_local 不跨线程)
static MESSAGES_TX: Mutex<Option<mpsc::Sender<Message>>> = Mutex::new(None);

fn send_message(message: Message) {
    let tx = MESSAGES_TX.lock().unwrap();
    tx.as_ref().unwrap().send(message).unwrap();
}

// ---- 日志(鸿蒙 hilog) ----
#[cfg(target_env = "ohos")]
extern "C" {
    fn OH_LOG_Print(level: i32, domain: u32, tag: *const core::ffi::c_char, fmt: *const core::ffi::c_char, ...);
    fn OH_LOG_DEBUG(domain: u32, tag: *const core::ffi::c_char, fmt: *const core::ffi::c_char, ...);
}

fn log_msg(tag: &str, msg: &str) {
    use std::ffi::CString;
    let tag_c = CString::new(tag).unwrap_or_default();
    let msg_c = CString::new(msg).unwrap_or_default();
    unsafe {
        // OH_LOG_Print(LOG_DEBUG=0, domain, tag, fmt, ...) 变参在 Rust 里不便,
        // 这里用 stderr 兜底(鸿蒙 native 进程可观测), 后续可接 hilog。
        eprintln!("[ohos::{}] {}", tag_c.to_str().unwrap_or(""), msg_c.to_str().unwrap_or(""));
    }
}

pub unsafe fn console_debug(msg: *const core::ffi::c_char) {
    use std::ffi::CStr;
    let s = CStr::from_ptr(msg).to_string_lossy();
    log_msg("debug", &s);
}
// console_info / warn / error 同 console_debug, 鸿蒙统一走 eprintln 暂
macro_rules! console_level {
    ($name:ident, $lvl:literal) => {
        pub unsafe fn $name(msg: *const core::ffi::c_char) {
            use std::ffi::CStr;
            let s = CStr::from_ptr(msg).to_string_lossy();
            log_msg($lvl, &s);
        }
    };
}
console_level!(console_info, "info");
console_level!(console_warn, "warn");
console_level!(console_error, "error");

/// 渲染线程状态(仿 android::MainThreadState, 去掉 JNI/AAsset)。
struct MainThreadState {
    libegl: LibEgl,
    egl_display: egl::EGLDisplay,
    egl_config: egl::EGLConfig,
    egl_context: egl::EGLContext,
    surface: egl::EGLSurface,
    window: *mut core::ffi::c_void,
    event_handler: Box<dyn EventHandler>,
    quit: bool,
    update_requested: bool,
    keymods: KeyMods,
}

impl MainThreadState {
    unsafe fn destroy_surface(&mut self) {
        if !self.surface.is_null() {
            (self.libegl.eglMakeCurrent)(
                self.egl_display,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            (self.libegl.eglDestroySurface)(self.egl_display, self.surface);
            self.surface = std::ptr::null_mut();
        }
    }

    unsafe fn update_surface(&mut self, window: *mut core::ffi::c_void) {
        self.window = window;
        self.destroy_surface();
        self.surface = (self.libegl.eglCreateWindowSurface)(
            self.egl_display,
            self.egl_config,
            window as _,
            std::ptr::null_mut(),
        );
        if self.surface.is_null() {
            // 记录到文件(渲染线程无 hilog 通道)
            std::fs::write("/data/local/tmp/pet_render.log", "eglCreateWindowSurface failed\n").ok();
            return;
        }
        let res = (self.libegl.eglMakeCurrent)(
            self.egl_display,
            self.surface,
            self.surface,
            self.egl_context,
        );
        if res == 0 {
            std::fs::write("/data/local/tmp/pet_render.log", "eglMakeCurrent failed\n").ok();
        }
    }

    fn process_message(&mut self, msg: Message) {
        match msg {
            Message::SurfaceCreated { window } => unsafe {
                self.update_surface(window);
            },
            Message::SurfaceDestroyed => unsafe {
                self.destroy_surface();
            },
            Message::SurfaceChanged { width, height } => {
                {
                    let mut d = crate::native_display().lock().unwrap();
                    d.screen_width = width as _;
                    d.screen_height = height as _;
                }
                // 尺寸变化(XComponent 缩放/键盘避让) → 重建 EGL surface 跟随新尺寸
                if width > 0 && height > 0 && !self.window.is_null() {
                    unsafe {
                        self.update_surface(self.window);
                    }
                }
                self.event_handler.resize_event(width as _, height as _);
            }
            Message::Touch { phase, touch_id, x, y } => {
                self.event_handler.touch_event(phase, touch_id, x, y);
            }
            Message::Character { character } => {
                if let Some(character) = char::from_u32(character) {
                    self.event_handler.char_event(character, Default::default(), false);
                }
            }
            Message::KeyDown { keycode } => {
                match keycode {
                    KeyCode::LeftShift | KeyCode::RightShift => self.keymods.shift = true,
                    KeyCode::LeftControl | KeyCode::RightControl => self.keymods.ctrl = true,
                    KeyCode::LeftAlt | KeyCode::RightAlt => self.keymods.alt = true,
                    KeyCode::LeftSuper | KeyCode::RightSuper => self.keymods.logo = true,
                    _ => {}
                }
                self.event_handler.key_down_event(keycode, self.keymods, false);
            }
            Message::KeyUp { keycode } => {
                match keycode {
                    KeyCode::LeftShift | KeyCode::RightShift => self.keymods.shift = false,
                    KeyCode::LeftControl | KeyCode::RightControl => self.keymods.ctrl = false,
                    KeyCode::LeftAlt | KeyCode::RightAlt => self.keymods.alt = false,
                    KeyCode::LeftSuper | KeyCode::RightSuper => self.keymods.logo = false,
                    _ => {}
                }
                self.event_handler.key_up_event(keycode, self.keymods);
            }
            Message::Pause => self.event_handler.window_minimized_event(),
            Message::Resume => self.event_handler.window_restored_event(),
            Message::Destroy => {
                self.quit = true;
                self.event_handler.quit_requested_event()
            }
            Message::Request(req) => self.process_request(req),
        }
    }

    fn frame(&mut self) {
        self.event_handler.update();
        if !self.surface.is_null() {
            self.update_requested = false;
            self.event_handler.draw();
            unsafe {
                (self.libegl.eglSwapBuffers)(self.egl_display, self.surface);
            }
        }
    }

    fn process_request(&mut self, request: crate::native::Request) {
        use crate::native::Request::*;
        match request {
            ScheduleUpdate => self.update_requested = true,
            // 鸿蒙全屏/键盘由 ArkTS 宿主管理, native 侧暂不处理
            _ => {}
        }
    }
}

/// 简版剪贴板: 鸿蒙无统一原生剪贴板 API, 先返回 None / 空实现。
pub struct OhosClipboard {}
impl OhosClipboard {
    pub fn new() -> OhosClipboard {
        OhosClipboard {}
    }
}
impl crate::native::Clipboard for OhosClipboard {
    fn get(&mut self) -> Option<String> {
        None
    }
    fn set(&mut self, _string: &str) {}
}

/// 渲染线程入口(仿 android::run)。
///
/// 宿主侧流程(ArkTS + NAPI):
/// 1. `XComponent` 的 `onSurfaceCreated` 回调 → 调用本 crate 导出的
///    `ohos_surface_created(surface)`(surface=OHNativeWindow*)。
/// 2. `onSurfaceChanged(w,h)` → `ohos_surface_changed(w,h)`。
/// 3. `onSurfaceDestroyed()` → `ohos_surface_destroyed()`。
/// 这些导出符号见下方 `#[no_mangle]` 函数, 由 NAPI 包装后暴露给 ArkTS。
pub unsafe fn run<F>(conf: crate::conf::Conf, f: F)
where
    F: 'static + FnOnce() -> Box<dyn EventHandler>,
{
    struct SendHack<F>(F);
    unsafe impl<F> Send for SendHack<F> {}

    let f = SendHack(f);
    let (tx, rx) = mpsc::channel();
    let tx2 = tx.clone();
    *MESSAGES_TX.lock().unwrap() = Some(tx2);

    thread::spawn(move || {
        let mut libegl = LibEgl::try_load().expect("Cant load LibEGL");

        // 等第一个 surface(仿 android: 有时应用启动有权限弹窗, 需等窗口真正可用)
        let window = 'a: loop {
            match rx.try_recv() {
                Ok(Message::SurfaceCreated { window }) => break 'a window,
                _ => {}
            }
        };
        let (screen_width, screen_height) = 'a: loop {
            match rx.try_recv() {
                Ok(Message::SurfaceChanged { width, height }) => break 'a (width as f32, height as f32),
                _ => {}
            }
        };

        let (egl_context, egl_config, egl_display) = crate::native::egl::create_egl_context(
            &mut libegl,
            std::ptr::null_mut(), /* EGL_DEFAULT_DISPLAY */
            conf.platform.framebuffer_alpha,
            conf.sample_count,
        )
        .expect("Cant create EGL context");

        assert!(!egl_display.is_null());
        assert!(!egl_config.is_null());

        crate::native::gl::load_gl_funcs(|proc| {
            let name = std::ffi::CString::new(proc).unwrap();
            (libegl.eglGetProcAddress)(name.as_ptr() as _)
        });

        let surface = (libegl.eglCreateWindowSurface)(
            egl_display,
            egl_config,
            window as _,
            std::ptr::null_mut(),
        );
        assert!(!surface.is_null());

        if (libegl.eglMakeCurrent)(egl_display, surface, surface, egl_context) == 0 {
            panic!("eglMakeCurrent failed");
        }

        let clipboard = Box::new(OhosClipboard::new());
        // ohos 复用非 android 的 mpsc::Sender<Request> 通道(仿 linux/windows):
        // 独立 request 通道, 渲染循环里转发给 process_request
        let (req_tx, req_rx): (mpsc::Sender<crate::native::Request>, mpsc::Receiver<crate::native::Request>) = mpsc::channel();
        crate::set_or_replace_display(NativeDisplayData {
            high_dpi: conf.high_dpi,
            blocking_event_loop: conf.platform.blocking_event_loop,
            ..NativeDisplayData::new(screen_width as _, screen_height as _, req_tx, clipboard)
        });

        let event_handler = f.0();
        let mut s = MainThreadState {
            libegl,
            egl_display,
            egl_config,
            egl_context,
            surface,
            window,
            event_handler,
            quit: false,
            update_requested: true,
            keymods: KeyMods { shift: false, ctrl: false, alt: false, logo: false },
        };

        let rx_timeout = conf
            .platform
            .sleep_interval_ms
            .map(|sleep| Duration::from_millis(sleep as u64));

        while !s.quit {
            let block_on_wait = conf.platform.blocking_event_loop && !s.update_requested;
            if block_on_wait {
                match rx_recv(&rx, rx_timeout) {
                    Ok(msg) => s.process_message(msg),
                    Err(mpsc::RecvTimeoutError::Timeout) => s.update_requested = true,
                    Err(mpsc::RecvTimeoutError::Disconnected) => panic!(),
                }
            } else {
                while let Ok(msg) = rx.try_recv() {
                    s.process_message(msg);
                }
                while let Ok(req) = req_rx.try_recv() {
                    s.process_request(req);
                }
            }
            if !conf.platform.blocking_event_loop || s.update_requested {
                s.frame();
            }
            thread::yield_now();
        }

        (s.libegl.eglMakeCurrent)(
            s.egl_display,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        (s.libegl.eglDestroySurface)(s.egl_display, s.surface);
        (s.libegl.eglDestroyContext)(s.egl_display, s.egl_context);
        (s.libegl.eglTerminate)(s.egl_display);
    });
}

fn rx_recv(rx: &mpsc::Receiver<Message>, timeout: Option<Duration>) -> Result<Message, mpsc::RecvTimeoutError> {
    match timeout {
        Some(d) => rx.recv_timeout(d),
        None => rx.recv().map_err(|_| mpsc::RecvTimeoutError::Disconnected),
    }
}

// ---- 宿主(ArkTS/NAPI)调用的导出符号 ----
// 这些由 NAPI 包装暴露给 ArkTS XComponent 回调; 在 surface 事件到达前不渲染。
// 注意: quad_main() 由 macroquad 生成(native lib 入口), 与 android 一致。

extern "C" {
    fn quad_main();
}

/// XComponent surface 创建: surface = OHNativeWindow* (从 nativeXComponent 或
/// XComponent 的 surface 获取)。由宿主 NAPI 封装 ohos_surface_created 调用。
#[no_mangle]
pub unsafe extern "C" fn ohos_surface_created(surface: *mut core::ffi::c_void) {
    if surface.is_null() {
        log_msg("surface", "ohos_surface_created: null surface ignored");
        return;
    }
    send_message(Message::SurfaceCreated { window: surface });
}

#[no_mangle]
pub unsafe extern "C" fn ohos_surface_changed(width: i32, height: i32) {
    send_message(Message::SurfaceChanged { width, height });
}

#[no_mangle]
pub unsafe extern "C" fn ohos_surface_destroyed() {
    send_message(Message::SurfaceDestroyed);
}

#[no_mangle]
pub unsafe extern "C" fn ohos_touch(x: f32, y: f32, touch_id: u64, down: bool) {
    let phase = if down { TouchPhase::Started } else { TouchPhase::Ended };
    send_message(Message::Touch { phase, touch_id, x, y });
}

#[no_mangle]
pub unsafe extern "C" fn ohos_char(ch: u32) {
    send_message(Message::Character { character: ch });
}

/// 特殊键(退格/回车/Tab 等): keycode 为鸿蒙 KeyCode 值(oh_key_code.h)。
/// 普通字符走 ohos_char(unicode/keyText 映射)。
#[no_mangle]
pub unsafe extern "C" fn ohos_key(keycode: i32, down: bool) {
    let kc = match keycode {
        2049 => KeyCode::Tab,
        2054 => KeyCode::Enter,
        2055 => KeyCode::Backspace,
        _ => return, // 其它键暂不转发(输入框只需这几个)
    };
    let msg = if down {
        Message::KeyDown { keycode: kc }
    } else {
        Message::KeyUp { keycode: kc }
    };
    send_message(msg);
}

/// Pause/Resume: 宿主在页面 onHide/onShow 时调用。
#[no_mangle]
pub unsafe extern "C" fn ohos_pause() {
    send_message(Message::Pause);
}
#[no_mangle]
pub unsafe extern "C" fn ohos_resume() {
    send_message(Message::Resume);
}
