//! GDI 极简桌宠(参考 catime): 分层透明窗口 + GDI 立绘渲染, 目标内存 <3MB。
//! raw FFI 直接调 Win32(无 windows-rs 类型封装, 与 C 等价)。
//! 立绘为预合成 32 位 BGRA BMP(300x550, 660KB/张), GDI 直接加载。
#![allow(non_snake_case, dead_code)]
use std::os::raw::c_void;
use std::ptr;

// ---------- Win32 声明(raw FFI) ----------
#[link(name = "user32")]
extern "system" {
    fn GetDC(hWnd: *mut c_void) -> *mut c_void;
    fn ReleaseDC(hWnd: *mut c_void, hDC: *mut c_void) -> i32;
    fn CreateCompatibleDC(hDC: *mut c_void) -> *mut c_void;
    fn DeleteDC(hDC: *mut c_void) -> i32;
    fn RegisterClassW(wc: *const WNDCLASSW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32, lpClassName: *const u16, lpWindowName: *const u16,
        dwStyle: u32, x: i32, y: i32, nWidth: i32, nHeight: i32,
        hWndParent: *mut c_void, hMenu: *mut c_void, hInstance: *mut c_void, lpParam: *mut c_void,
    ) -> *mut c_void;
    fn DefWindowProcW(hWnd: *mut c_void, msg: u32, wParam: usize, lParam: isize) -> isize;
    fn GetMessageW(lpMsg: *mut MSG, hWnd: *mut c_void, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageW(lpMsg: *const MSG) -> isize;
    fn SetTimer(hWnd: *mut c_void, nIDEvent: usize, uElapse: u32, lpTimerFunc: Option<unsafe extern "system" fn(*mut c_void, u32, usize, u32)>) -> usize;
    fn SetWindowPos(hWnd: *mut c_void, hWndInsertAfter: *mut c_void, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn SetWindowLongPtrW(hWnd: *mut c_void, nIndex: i32, dwNewLong: isize) -> isize;
    fn GetWindowLongPtrW(hWnd: *mut c_void, nIndex: i32) -> isize;
    fn PostQuitMessage(nExitCode: i32);
    fn InvalidateRect(hWnd: *mut c_void, rect: *const c_void, bErase: i32) -> i32;
    fn ValidateRect(hWnd: *mut c_void, rect: *const c_void) -> i32;
    fn GetModuleHandleW(lpModuleName: *const u16) -> *mut c_void;
    fn GetModuleFileNameW(hModule: *mut c_void, lpFilename: *mut u16, nSize: u32) -> u32;
    fn LoadCursorW(hInstance: *mut c_void, lpCursorName: *const u16) -> *mut c_void;
    fn UpdateLayeredWindow(
        hWnd: *mut c_void, hdcDst: *mut c_void, pptDst: *mut POINT, psize: *mut SIZE,
        hdcSrc: *mut c_void, pptSrc: *mut POINT, crKey: u32, pblend: *const BLENDFUNCTION, dwFlags: u32,
    ) -> i32;
    fn SelectObject(hDC: *mut c_void, h: *mut c_void) -> *mut c_void;
    fn DeleteObject(h: *mut c_void) -> i32;
    fn CreateDIBSection(
        hdc: *mut c_void, pbmi: *const BITMAPINFO, usage: u32, ppvBits: *mut *mut c_void,
        hSection: *mut c_void, offset: u32,
    ) -> *mut c_void;
}

#[link(name = "gdi32")]
extern "system" {
    // 上面 user32 里声明了 CreateCompatibleDC/DeleteDC(实际在 gdi32, 但链接名不影响调用)
}

#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(hProcess: *mut c_void, ppsmemCounters: *mut PROCESS_MEMORY_COUNTERS, cb: u32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn GetLastError() -> u32;
    fn LoadLibraryW(lpLibFileName: *const u16) -> *mut c_void;
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const u8) -> *mut c_void;
    fn FreeLibrary(hLibModule: *mut c_void) -> i32;
}

// PlaySoundW 动态加载(避免静态链接把 WINMM 拖进常驻内存)
type PlaySoundFn = unsafe extern "system" fn(*const u8, *mut c_void, u32) -> i32;
static mut PLAY_SOUND_FN: Option<PlaySoundFn> = None;
static mut WINMM_HMODULE: *mut c_void = ptr::null_mut();

/// 播放内存中的 wav 数据(SND_MEMORY, 嵌入资源, 数据 'static 安全)。
unsafe fn play_sound_mem(data: &'static [u8]) -> i32 {
    if PLAY_SOUND_FN.is_none() {
        let winmm: Vec<u16> = "winmm.dll".encode_utf16().chain(std::iter::once(0)).collect();
        let m = LoadLibraryW(winmm.as_ptr());
        if !m.is_null() {
            let p = GetProcAddress(m, b"PlaySoundW\0".as_ptr() as *const u8);
            if !p.is_null() {
                WINMM_HMODULE = m;
                PLAY_SOUND_FN = Some(std::mem::transmute(p));
            }
        }
    }
    match PLAY_SOUND_FN {
        Some(f) => f(data.as_ptr(), ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT),
        None => 0,
    }
}

/// 播放结束(约 6s 后由 WM_TIMER 调用): 卸载 winmm 音频栈, 内存回落。
/// 引用计数: 播放中(未结束)不卸载; 被卸载后下次播放会重新加载。
unsafe fn unload_sound_dyn() {
    // 先卸载依赖音频 DLL(它们由 winmm 加载, 播放结束后可释放)
    for name in ["MMDevAPI.DLL\0", "AUDIOSES.DLL\0", "winmmbase.dll\0"] {
        let w: Vec<u16> = name.encode_utf16().collect();
        let m = GetModuleHandleW(w.as_ptr());
        if !m.is_null() {
            for _ in 0..4 {
                if FreeLibrary(m) == 0 {
                    break;
                }
            }
        }
    }
    for _ in 0..4 {
        if WINMM_HMODULE.is_null() {
            break;
        }
        FreeLibrary(WINMM_HMODULE);
    }
    WINMM_HMODULE = ptr::null_mut();
    PLAY_SOUND_FN = None;
}

const SND_ASYNC: u32 = 0x0001;
const SND_NODEFAULT: u32 = 0x0002;
const SND_MEMORY: u32 = 0x0004;

// ---------- 嵌入资源(单文件分发, 无外部 assets 依赖) ----------
static PET_BMP: &[u8] = include_bytes!("../assets/pet.bmp");
static EYE_PATCH_BMP: &[u8] = include_bytes!("../assets/eye_patch.bmp");
static GREETING_WAVS: [&[u8]; 10] = [
    include_bytes!("../assets/voice/greeting_01.wav"),
    include_bytes!("../assets/voice/greeting_02.wav"),
    include_bytes!("../assets/voice/greeting_03.wav"),
    include_bytes!("../assets/voice/greeting_04.wav"),
    include_bytes!("../assets/voice/greeting_05.wav"),
    include_bytes!("../assets/voice/greeting_06.wav"),
    include_bytes!("../assets/voice/greeting_07.wav"),
    include_bytes!("../assets/voice/greeting_08.wav"),
    include_bytes!("../assets/voice/greeting_09.wav"),
    include_bytes!("../assets/voice/greeting_10.wav"),
];

// ---------- 结构 ----------
#[repr(C)]
struct WNDCLASSW {
    style: u32,
    lpfnWndProc: Option<unsafe extern "system" fn(*mut c_void, u32, usize, isize) -> isize>,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: *mut c_void,
    hIcon: *mut c_void,
    hCursor: *mut c_void,
    hbrBackground: *mut c_void,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
}
#[repr(C)]
struct MSG {
    hwnd: *mut c_void,
    message: u32,
    wParam: usize,
    lParam: isize,
    time: u32,
    pt: POINT,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct POINT {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct SIZE {
    cx: i32,
    cy: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct BLENDFUNCTION {
    BlendOp: u8,
    BlendFlags: u8,
    SourceConstantAlpha: u8,
    AlphaFormat: u8,
}
#[repr(C)]
struct BITMAPINFOHEADER {
    biSize: u32,
    biWidth: i32,
    biHeight: i32,
    biPlanes: u16,
    biBitCount: u16,
    biCompression: u32,
    biSizeImage: u32,
    biXPelsPerMeter: i32,
    biYPelsPerMeter: i32,
    biClrUsed: u32,
    biClrImportant: u32,
}
#[repr(C)]
struct BITMAPINFO {
    bmiHeader: BITMAPINFOHEADER,
    bmiColors: [u32; 1],
}
#[repr(C)]
struct PROCESS_MEMORY_COUNTERS {
    cb: u32,
    PageFaultCount: u32,
    PeakWorkingSetSize: usize,
    WorkingSetSize: usize,
    QuotaPeakPagedPoolUsage: usize,
    QuotaPagedPoolUsage: usize,
    QuotaPeakNonPagedPoolUsage: usize,
    QuotaNonPagedPoolUsage: usize,
    PagefileUsage: usize,
    PeakPagefileUsage: usize,
}

const WS_POPUP: u32 = 0x8000_0000;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const GWLP_USERDATA: i32 = -21;
const WM_TIMER: u32 = 0x0113;
const WM_PAINT: u32 = 0x000F;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const CW_USEDEFAULT: i32 = -2147483648; // 0x8000_0000
const HWND_TOPMOST: *mut c_void = -1isize as *mut c_void;
const HWND_TOP: *mut c_void = 0isize as *mut c_void;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_SHOWWINDOW: u32 = 0x0040;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_NOMOVE: u32 = 0x0002;
const IDC_ARROW: *const u16 = 32512usize as *const u16;
const ULW_ALPHA: u32 = 0x0000_0002;
const AC_SRC_OVER: u8 = 0;
const AC_SRC_ALPHA: u8 = 1;
const BI_RGB: u32 = 0;
const DIB_RGB_COLORS: u32 = 0;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_KEYDOWN: u32 = 0x0100;
const WM_CHAR: u32 = 0x0102;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_NCHITTEST: u32 = 0x0084;
const HTCAPTION: isize = 2;
const WM_SYSCOMMAND: u32 = 0x0112;
const SC_MOVE: u32 = 0xF010;
const WM_QUIT: u32 = 0x0012;

const WIN_W: i32 = 250;
const WIN_H: i32 = 450;
const FRAME_NORMAL: usize = 0;
const FRAME_BLINK: usize = 1;

struct PetWindow {
    frames: Vec<Vec<u8>>,     // 正常帧(1 张常驻)
    eye_patch: Vec<u8>,       // 闭眼补丁(眼睛区域)
    patch_x: i32,
    patch_y: i32,
    patch_w: i32,
    patch_h: i32,
    frame: usize,
    last_swap: u128,
    voice_idx: usize,
    last_play: u128,            // 最近一次语音播放时刻(用于播放后卸载 winmm)
    bubble: Option<(u16, u128)>, // (台词索引, 显示到此刻)
    reply: Option<(String, u128)>, // 聊天回复文本(显示到此刻)
    canvas: GdiCanvas,          // 复用 GDI 对象(DIB/DC), 防泄漏
    input: String,              // 聊天输入缓冲
    input_mode: bool,           // 输入模式(Enter 进入/提交)
}

/// 复用的 GDI 画布: memDC + DIB(像素缓冲), 每帧只更新像素, 不重建。
struct GdiCanvas {
    mem_dc: *mut c_void,
    dib: *mut c_void,
    bits: *mut u8,
}

impl GdiCanvas {
    unsafe fn new(w: i32, h: i32) -> Self {
        let screen_dc = GetDC(ptr::null_mut());
        let mem_dc = CreateCompatibleDC(screen_dc);
        ReleaseDC(ptr::null_mut(), screen_dc);
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut c_void = ptr::null_mut();
        let dib = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        SelectObject(mem_dc, dib);
        GdiCanvas { mem_dc, dib, bits: bits as *mut u8 }
    }
}

/// 丛雨中文问候台词(与 voice/greeting_XX.wav 一一对应)
const LINES: [&str; 10] = [
    "你好呀，吾辈是丛雨！",
    "早上好，主人！",
    "中午好，今天也要加油哦！",
    "晚上好，吾辈一直在等着你。",
    "辛苦了，吾辈给你揉揉肩！",
    "再见啦，下次再来找吾辈玩！",
    "你回来啦，吾辈好想你！",
    "别熬夜啦，要注意身体！",
    "今天心情怎么样？",
    "吾辈最喜欢你了！",
];

/// 聊天预置问答(输入匹配关键词 → 回复)
const DIALOG: [(&str, &str); 8] = [
    ("在吗", "在呢 怎么了"),
    ("吃饭", "还没呢 等会儿去吃"),
    ("想我", "嗯 有一点吧"),
    ("心情", "别难过 吾辈一直陪着主人"),
    ("晚安", "晚安 主人做个好梦"),
    ("陪我", "会一直陪着主人哒"),
    ("干嘛", "在刷视频"),
    ("喜欢", "喜欢主人呀"),
];

/// 解析 32 位 BGRA BMP 内存数据 → 自顶向下像素。
fn load_bmp_bytes(data: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    if data.len() < 54 {
        return None;
    }
    let pixel_offset = u32::from_le_bytes(data[10..14].try_into().ok()?) as usize;
    let row_size = ((w * 4) as usize + 3) & !3;
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    for y in 0..h as usize {
        let src = pixel_offset + (h as usize - 1 - y) * row_size;
        let dst = y * (w as usize) * 4;
        if src + w as usize * 4 <= data.len() {
            pixels[dst..dst + w as usize * 4].copy_from_slice(&data[src..src + w as usize * 4]);
        }
    }
    Some(pixels)
}

/// 分层窗口渲染: 复用 canvas(DIB/DC), 更新像素 + 气泡/输入/眨眼后 UpdateLayeredWindow。
unsafe fn render_frame(
    hwnd: *mut c_void, canvas: &GdiCanvas, pixels: &[u8], w: i32, h: i32,
    bubble: Option<(&str, usize)>, input: Option<&str>,
    blink: bool, patch: &[u8], px: i32, py: i32, pw: i32, ph: i32,
) {
    if !canvas.bits.is_null() {
        ptr::copy_nonoverlapping(pixels.as_ptr(), canvas.bits, pixels.len());
        // 眨眼: 叠加闭眼补丁(直接操作 DIB)
        if blink {
            let slice = std::slice::from_raw_parts_mut(canvas.bits, (w * h * 4) as usize);
            blend_patch(slice, w, patch, px, py, pw, ph);
        }
        // 台词/回复气泡: 立绘顶部半透明黑底 + 白字
        if let Some((text, _idx)) = bubble {
            let slice = std::slice::from_raw_parts_mut(canvas.bits, (w * h * 4) as usize);
            let bh = 40i32;
            let by = 8i32;
            blend_rect(slice, w, h, 8, by, w - 16, bh, [0, 0, 0, 200]);
            let font = CreateFontW(
                -16, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH,
                "Microsoft YaHei".encode_utf16().collect::<Vec<u16>>().as_ptr(),
            );
            let old_font = SelectObject(canvas.mem_dc, font);
            SetBkMode(canvas.mem_dc, TRANSPARENT);
            SetTextColor(canvas.mem_dc, RGB_WHITE);
            let mut rc = RECT { left: 8, top: by, right: w - 8, bottom: by + bh };
            let text_utf16: Vec<u16> = text.encode_utf16().collect();
            DrawTextW(canvas.mem_dc, text_utf16.as_ptr(), text_utf16.len() as i32, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            SelectObject(canvas.mem_dc, old_font);
            DeleteObject(font);
        }
        // 输入区: 立绘下方(输入模式显示已输入文本)
        if let Some(text) = input {
            let slice = std::slice::from_raw_parts_mut(canvas.bits, (w * h * 4) as usize);
            let bh = 34i32;
            let by = h - 44;
            blend_rect(slice, w, h, 8, by, w - 16, bh, [0, 0, 0, 180]);
            let font = CreateFontW(
                -15, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, DEFAULT_PITCH,
                "Microsoft YaHei".encode_utf16().collect::<Vec<u16>>().as_ptr(),
            );
            let old_font = SelectObject(canvas.mem_dc, font);
            SetBkMode(canvas.mem_dc, TRANSPARENT);
            SetTextColor(canvas.mem_dc, RGB_WHITE);
            let mut rc = RECT { left: 12, top: by, right: w - 12, bottom: by + bh };
            let text_utf16: Vec<u16> = text.encode_utf16().collect();
            if !text_utf16.is_empty() {
                DrawTextW(canvas.mem_dc, text_utf16.as_ptr(), text_utf16.len() as i32, &mut rc, DT_LEFT | DT_VCENTER | DT_SINGLELINE);
            }
            SelectObject(canvas.mem_dc, old_font);
            DeleteObject(font);
        }
    }
    let screen_dc = GetDC(ptr::null_mut());
    // ptDst = NULL: 保留 SetWindowPos 设置的窗口位置(不再把窗口拉到 (0,0))
    let mut size = SIZE { cx: w, cy: h };
    let mut src = POINT { x: 0, y: 0 };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA,
    };
    UpdateLayeredWindow(hwnd, screen_dc, ptr::null_mut(), &mut size, canvas.mem_dc, &mut src, 0, &blend, ULW_ALPHA);
    ReleaseDC(ptr::null_mut(), screen_dc);
}

/// 叠加闭眼补丁到帧像素(alpha blend)。
fn blend_patch(frame: &mut [u8], w: i32, patch: &[u8], px: i32, py: i32, pw: i32, ph: i32) {
    for y in 0..ph {
        for x in 0..pw {
            let si = (y * pw + x) as usize * 4;
            let a = patch[si + 3] as f32 / 255.0;
            if a <= 0.02 {
                continue;
            }
            let fx = px + x;
            let fy = py + y;
            if fx < 0 || fy < 0 || fx >= w || fy >= 450 {
                continue;
            }
            let di = (fy * w + fx) as usize * 4;
            frame[di] = (patch[si] as f32 * a + frame[di] as f32 * (1.0 - a)) as u8;
            frame[di + 1] = (patch[si + 1] as f32 * a + frame[di + 1] as f32 * (1.0 - a)) as u8;
            frame[di + 2] = (patch[si + 2] as f32 * a + frame[di + 2] as f32 * (1.0 - a)) as u8;
            frame[di + 3] = 255;
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        WM_TIMER => {
            let p = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PetWindow).as_mut();
            if let Some(pet) = p {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                // 气泡 3s / 回复 3.5s 超时消失
                if let Some((_, until)) = pet.bubble {
                    if now > until {
                        pet.bubble = None;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                }
                if let Some((_, until)) = &pet.reply {
                    if now > *until {
                        pet.reply = None;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                }
                // 播放完成(6s 后)卸载 winmm 音频栈, 内存回落
                if !WINMM_HMODULE.is_null() && now - pet.last_play > 6000 {
                    unload_sound_dyn();
                }
                // 眨眼: 闭眼 120ms, 每 3s 一次 (先判恢复再判触发, 避免 BLINK 分支永远先匹配)
                if pet.frame == FRAME_BLINK {
                    if now - pet.last_swap >= 120 {
                        pet.frame = FRAME_NORMAL;
                        pet.last_swap = now;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                } else if now - pet.last_swap >= 3000 {
                    pet.frame = FRAME_BLINK;
                    pet.last_swap = now;
                    InvalidateRect(hwnd, ptr::null(), 0);
                }
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let p = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PetWindow).as_mut();
            if let Some(pet) = p {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                // 气泡文本: 回复优先(聊天), 否则台词(点击说话)
                let bubble: Option<(&str, usize)> = if let Some((text, until)) = &pet.reply {
                    if now <= *until { Some((text.as_str(), 99)) } else { None }
                } else {
                    pet.bubble
                        .filter(|(_, until)| now <= *until)
                        .map(|(idx, _)| (LINES[idx as usize], idx as usize))
                };
                let input = if pet.input_mode { Some(pet.input.as_str()) } else { None };
                // 眨眼: 直接叠加补丁到 canvas(无临时帧); 否则渲染正常帧
                let blink = pet.frame == FRAME_BLINK;
                render_frame(hwnd, &pet.canvas, &pet.frames[0], WIN_W, WIN_H, bubble, input, blink, &pet.eye_patch, pet.patch_x, pet.patch_y, pet.patch_w, pet.patch_h);
                ValidateRect(hwnd, ptr::null());
            }
            0
        }
        WM_LBUTTONDOWN => {
            // 点击: 播放轮播问候语音 + 台词气泡
            let p = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PetWindow).as_mut();
            if let Some(pet) = p {
                let idx = pet.voice_idx % 10;
                pet.voice_idx += 1;
                play_sound_mem(GREETING_WAVS[idx]);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                pet.last_play = now;
                pet.bubble = Some((idx as u16, now + 3000));
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            0
        }
        WM_KEYDOWN => {
            // Enter: 进入输入模式(空)或提交; Esc: 退出; Backspace: 删字
            let p = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PetWindow).as_mut();
            if let Some(pet) = p {
                match wparam {
                    0x0D => { // Enter
                        if pet.input_mode {
                            if !pet.input.trim().is_empty() {
                                reply_and_speak(hwnd, pet);
                            }
                            pet.input.clear();
                            pet.input_mode = false;
                        } else {
                            pet.input_mode = true;
                        }
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    0x1B => { // Esc
                        pet.input.clear();
                        pet.input_mode = false;
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    0x08 => { // Backspace
                        pet.input.pop();
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    _ => {}
                }
            }
            0
        }
        WM_CHAR => {
            let p = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PetWindow).as_mut();
            if let Some(pet) = p {
                if pet.input_mode {
                    let ch = wparam as u32;
                    if (0x20..=0x7E).contains(&ch) || ch > 0x7F {
                        if pet.input.chars().count() < 24 {
                            if let Some(c) = char::from_u32(ch) {
                                pet.input.push(c);
                                InvalidateRect(hwnd, ptr::null(), 0);
                            }
                        }
                    }
                }
            }
            0
        }
        WM_DESTROY | WM_CLOSE => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 聊天提交: preset 关键词匹配 → 回复气泡 + 语音。
unsafe fn reply_and_speak(hwnd: *mut c_void, pet: &mut PetWindow) {
    let text = pet.input.trim();
    let mut reply = None;
    for (kw, ans) in DIALOG.iter() {
        if text.contains(kw) {
            reply = Some(*ans);
            break;
        }
    }
    let reply = reply.unwrap_or("嗯…吾辈听不太懂，换句话说说？");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    pet.reply = Some((reply.to_string(), now + 3500));
    // 语音: 按输入哈希轮播问候
    let idx = (text.bytes().fold(0u16, |a, b| a.wrapping_add(b as u16)) % 10) as usize;
    play_sound_mem(GREETING_WAVS[idx]);
    pet.last_play = now;
}

#[link(name = "user32")]
extern "system" {
    fn SendMessageW(hWnd: *mut c_void, msg: u32, wParam: usize, lParam: isize) -> isize;
    fn DrawTextW(hdc: *mut c_void, lpchText: *const u16, cchText: i32, lprc: *mut RECT, format: u32) -> i32;
}
const WM_NCLBUTTONDOWN: u32 = 0x00A1;
const DT_CENTER: u32 = 0x0001;
const DT_LEFT: u32 = 0x0000;
const DT_VCENTER: u32 = 0x0004;
const DT_SINGLELINE: u32 = 0x0020;

#[repr(C)]
#[derive(Clone, Copy)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(
        cHeight: i32, cWidth: i32, cEscapement: i32, cOrientation: i32, cWeight: i32,
        bItalic: u32, bUnderline: u32, bStrikeOut: u32, iCharSet: u32, iOutPrecision: u32,
        iClipPrecision: u32, iQuality: u32, iPitchAndFamily: u32, pszFaceName: *const u16,
    ) -> *mut c_void;
    fn SetBkMode(hdc: *mut c_void, mode: i32) -> i32;
    fn SetTextColor(hdc: *mut c_void, color: u32) -> u32;
}
const TRANSPARENT: i32 = 1;
const FW_NORMAL: i32 = 400;
const DEFAULT_CHARSET: u32 = 1;
const OUT_DEFAULT_PRECIS: u32 = 0;
const CLIP_DEFAULT_PRECIS: u32 = 0;
const CLEARTYPE_QUALITY: u32 = 5;
const DEFAULT_PITCH: u32 = 0;
const RGB_WHITE: u32 = 0x00FF_FFFF;

/// 在 BGRA 像素上叠加半透明色块(气泡背景)。
fn blend_rect(pixels: &mut [u8], w: i32, h: i32, x: i32, y: i32, bw: i32, bh: i32, color: [u8; 4]) {
    for py in y..(y + bh) {
        if py < 0 || py >= h {
            continue;
        }
        for px in x..(x + bw) {
            if px < 0 || px >= w {
                continue;
            }
            let i = (py * w + px) as usize * 4;
            let a = color[3] as f32 / 255.0;
            pixels[i] = (color[0] as f32 * a + pixels[i] as f32 * (1.0 - a)) as u8;
            pixels[i + 1] = (color[1] as f32 * a + pixels[i + 1] as f32 * (1.0 - a)) as u8;
            pixels[i + 2] = (color[2] as f32 * a + pixels[i + 2] as f32 * (1.0 - a)) as u8;
            pixels[i + 3] = pixels[i + 3].max(color[3]);
        }
    }
}

fn working_set_kb() -> u64 {
    unsafe {
        let handle = GetCurrentProcess();
        let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(handle, &mut pmc, pmc.cb) != 0 {
            return pmc.WorkingSetSize as u64 / 1024;
        }
        0
    }
}

fn main() {
    unsafe {
        let hinstance = GetModuleHandleW(ptr::null());
        let class_name: Vec<u16> = "GdiPetWindow".encode_utf16().chain(std::iter::once(0)).collect();
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: ptr::null_mut(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        let reg = RegisterClassW(&wc);
        // 资源全部内嵌(exe 单文件, 任意 cwd 可启动)
        let f_normal = load_bmp_bytes(PET_BMP, 250, 450).unwrap_or_default();
        // 闭眼补丁(眼睛区域, 叠加到正常帧 = 眨眼, 免第二张全帧)
        let patch = load_bmp_bytes(EYE_PATCH_BMP, 54, 48).unwrap_or_default();
        let mut pet = PetWindow {
            frames: vec![f_normal],
            eye_patch: patch,
            patch_x: 88,
            patch_y: 36,
            patch_w: 54,
            patch_h: 48,
            frame: 0,
            last_swap: 0,
            voice_idx: 0,
            last_play: 0,
            bubble: None,
            reply: None,
            input: String::new(),
            input_mode: false,
            canvas: GdiCanvas { mem_dc: ptr::null_mut(), dib: ptr::null_mut(), bits: ptr::null_mut() },
        };

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            class_name.as_ptr(),
            WS_POPUP,
            CW_USEDEFAULT, CW_USEDEFAULT, WIN_W, WIN_H,
            ptr::null_mut(), ptr::null_mut(), hinstance, ptr::null_mut(),
        );
        // 窗口创建后再初始化 canvas(GDI 对象)
        pet.canvas = GdiCanvas::new(WIN_W, WIN_H);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut pet as *mut PetWindow as isize);
        SetWindowPos(hwnd, HWND_TOPMOST, 1300, 120, 0, 0, SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE);
        SetTimer(hwnd, 1, 50, None);

        let mut msg = MSG::zeroed();
        let mut printed = false;
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            if !printed {
                println!("WorkingSet: {} KB", working_set_kb());
                printed = true;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

impl MSG {
    fn zeroed() -> Self {
        unsafe { std::mem::zeroed() }
    }
}
