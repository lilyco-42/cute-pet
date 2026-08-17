//! OHOS(OpenHarmony) 音频后端: OH_AudioRenderer (WRITE 回调模式) + mixer。
//!
//! 结构对齐 alsa_snd: AudioContext::new() 建 mixer 并启动 OH_AudioRenderer,
//! 系统在需要数据时回调 on_write_data, 从 mixer 混音 f32 stereo 44100Hz
//! 直接填缓冲(SAMPLE_F32LE, 无需格式转换)。
//!
//! API 来源: SDK 6.1 的 ohaudio 头文件(native_audiostreambuilder.h /
//! native_audiorenderer.h); WRITE 模式用 SetRendererWriteDataCallback
//! (SDK 6.1 已移除 OH_AudioRenderer_Write 直写函数)。

use crate::error::Error;
use crate::mixer::{Mixer, MixerControl};
use crate::PlaySoundParams;

use std::ffi::c_void;
use std::sync::Mutex;

pub use crate::mixer::Playback;

mod consts {
    // 48000: OH_AudioRenderer 默认采样率(不调 SetSampleRate — SDK 6.1
    // stub libohaudio 未导出该符号); 与系统默认对齐避免变速
    pub const RATE: u32 = 48000;
    pub const CHANNELS: u32 = 2;
    // 单样本字节: 2ch * f32(4B)
    pub const SAMPLE_BYTES: usize = CHANNELS as usize * 4;
    pub const FRAMES_PER_CALLBACK: usize = 1024;
}

// ---- OH_AudioRenderer FFI(libohaudio.so) ----
const AUDIOSTREAM_TYPE_RENDERER: i32 = 1;
const AUDIOSTREAM_USAGE_MUSIC: i32 = 1;
const AUDIOSTREAM_LATENCY_MODE_NORMAL: i32 = 0;
const AUDIOSTREAM_SAMPLE_F32LE: i32 = 4;

#[repr(i32)]
enum CallbackResult {
    Valid = 0,
    Invalid = 1,
}

type OnWriteDataCb = unsafe extern "C" fn(
    renderer: *mut c_void,
    user_data: *mut c_void,
    audio_data: *mut c_void,
    audio_data_size: i32,
) -> CallbackResult;

#[link(name = "ohaudio")]
extern "C" {
    fn OH_AudioStreamBuilder_Create(builder: *mut *mut c_void, stream_type: i32) -> i32;
    fn OH_AudioStreamBuilder_Destroy(builder: *mut c_void) -> i32;
    fn OH_AudioStreamBuilder_SetChannelCount(builder: *mut c_void, channels: i32) -> i32;
    fn OH_AudioStreamBuilder_SetSampleFormat(builder: *mut c_void, format: i32) -> i32;
    fn OH_AudioStreamBuilder_SetRendererInfo(builder: *mut c_void, usage: i32) -> i32;
    fn OH_AudioStreamBuilder_SetLatencyMode(builder: *mut c_void, mode: i32) -> i32;
    fn OH_AudioStreamBuilder_SetRendererWriteDataCallback(
        builder: *mut c_void,
        callback: Option<OnWriteDataCb>,
        user_data: *mut c_void,
    ) -> i32;
    fn OH_AudioStreamBuilder_GenerateRenderer(builder: *mut c_void, renderer: *mut *mut c_void) -> i32;
    fn OH_AudioRenderer_Start(renderer: *mut c_void) -> i32;
    fn OH_AudioRenderer_Stop(renderer: *mut c_void) -> i32;
    fn OH_AudioRenderer_Release(renderer: *mut c_void) -> i32;
}

// ---- hilog 日志(验证音频链路) ----
#[link(name = "hilog_ndk.z")]
extern "C" {
    fn OH_LOG_Print(level: i32, domain: u32, tag: *const std::ffi::c_char, fmt: *const std::ffi::c_char, ...);
}
fn hlog(msg: &str) {
    use std::ffi::CString;
    let tag = CString::new("CutePetAudio").unwrap();
    let m = CString::new(msg).unwrap();
    unsafe { OH_LOG_Print(3 /* INFO */, 0xD003C00, tag.as_ptr(), b"%{public}s\0".as_ptr() as _, m.as_ptr()) }
}

// 回调共享的 mixer(跨线程: 音频线程回调 + 主线程 play/stop)
// AudioContext::new() 时写入 Some(mixer)
static SHARED_MIXER: Mutex<Option<Mixer>> = Mutex::new(None);

/// OH_AudioRenderer 数据回调: 系统请求 PCM, 从 mixer 混音填充。
unsafe extern "C" fn on_write_data(
    _renderer: *mut c_void,
    _user_data: *mut c_void,
    audio_data: *mut c_void,
    audio_data_size: i32,
) -> CallbackResult {
    let size = audio_data_size as usize;
    if size == 0 || audio_data.is_null() {
        return CallbackResult::Invalid;
    }
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if n < 5 || n % 1000 == 0 {
        hlog(&format!("on_write_data #{n} size={size}"));
    }
    // 帧数 = 字节 / 单样本字节; 回调保证是单样本整数倍
    let frames = size / consts::SAMPLE_BYTES;
    let mut buffer: Vec<f32> = vec![0.0; frames * consts::CHANNELS as usize];
    {
        let mut mixer = SHARED_MIXER.lock().unwrap();
        if let Some(m) = mixer.as_mut() {
            m.fill_audio_buffer(&mut buffer, frames);
        }
    }
    std::ptr::copy_nonoverlapping(buffer.as_ptr() as *const u8, audio_data as *mut u8, size);
    CallbackResult::Valid
}

pub struct AudioContext {
    pub(crate) mixer_ctrl: MixerControl,
    renderer: *mut c_void,
}

impl AudioContext {
    pub fn new() -> AudioContext {
        let (builder, mixer_ctrl) = Mixer::new();
        // 共享 mixer 供回调使用
        *SHARED_MIXER.lock().unwrap() = Some(builder.build());

        let renderer = unsafe { setup_renderer() };
        let ctx = AudioContext { mixer_ctrl, renderer };
        ctx
    }
}

impl Drop for AudioContext {
    fn drop(&mut self) {
        if !self.renderer.is_null() {
            unsafe {
                OH_AudioRenderer_Stop(self.renderer);
                OH_AudioRenderer_Release(self.renderer);
            }
            self.renderer = std::ptr::null_mut();
        }
    }
}

unsafe fn setup_renderer() -> *mut c_void {
    let mut builder: *mut c_void = std::ptr::null_mut();
    if OH_AudioStreamBuilder_Create(&mut builder, AUDIOSTREAM_TYPE_RENDERER) != 0 {
        hlog("builder create failed");
        return std::ptr::null_mut();
    }
    OH_AudioStreamBuilder_SetChannelCount(builder, consts::CHANNELS as i32);
    OH_AudioStreamBuilder_SetSampleFormat(builder, AUDIOSTREAM_SAMPLE_F32LE);
    OH_AudioStreamBuilder_SetRendererInfo(builder, AUDIOSTREAM_USAGE_MUSIC);
    OH_AudioStreamBuilder_SetLatencyMode(builder, AUDIOSTREAM_LATENCY_MODE_NORMAL);
    OH_AudioStreamBuilder_SetRendererWriteDataCallback(
        builder,
        Some(on_write_data),
        std::ptr::null_mut(),
    );

    let mut renderer: *mut c_void = std::ptr::null_mut();
    let ret = OH_AudioStreamBuilder_GenerateRenderer(builder, &mut renderer);
    OH_AudioStreamBuilder_Destroy(builder);
    if ret != 0 || renderer.is_null() {
        hlog(&format!("GenerateRenderer failed ret={ret}"));
        return std::ptr::null_mut();
    }
    let start_ret = OH_AudioRenderer_Start(renderer);
    hlog(&format!("renderer start ret={start_ret}"));
    renderer
}

impl Sound {
    pub fn load(ctx: &AudioContext, data: &[u8]) -> Sound {
        let sound_id = ctx.mixer_ctrl.load(data);
        Sound { sound_id }
    }

    pub fn play(&self, ctx: &AudioContext, params: PlaySoundParams) -> Playback {
        ctx.mixer_ctrl.play(self.sound_id, params)
    }

    pub fn stop(&self, ctx: &AudioContext) {
        ctx.mixer_ctrl.stop_all(self.sound_id);
    }

    pub fn set_volume(&self, ctx: &AudioContext, volume: f32) {
        ctx.mixer_ctrl.set_volume_all(self.sound_id, volume);
    }

    pub fn delete(&self, ctx: &AudioContext) {
        ctx.mixer_ctrl.delete(self.sound_id);
    }
}

pub struct Sound {
    sound_id: u32,
}

// 保持与其它平台一致的 Error 兼容(未用到, 避免未用警告)
#[allow(dead_code)]
fn _unused(_: Error) {}
