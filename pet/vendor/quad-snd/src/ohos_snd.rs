//! OHOS(OpenHarmony) 音频桩: 结构对齐 alsa_snd(含 mixer_ctrl/MixerControl 与
//! Sound::load/play 等), 播放按需接 OH_AudioRenderer。先在 ohos 下让主程序
//! 可链接(渲染通), 音频输出后续单独实现。
#![allow(dead_code)]

use crate::mixer::{Mixer, MixerControl};
use crate::PlaySoundParams;

// 与其它平台一致: Playback 由 mixer 提供(play() 返回 mixer::Playback)
pub use crate::mixer::Playback;

pub struct AudioContext {
    pub(crate) mixer_ctrl: MixerControl,
}

pub struct Sound {
    sound_id: u32,
}

impl AudioContext {
    pub fn new() -> AudioContext {
        // 暂不起音频线程(无 OH_AudioRenderer); mixer 通道仍建, 兼容后续接入
        let (_builder, mixer_ctrl) = Mixer::new();
        AudioContext { mixer_ctrl }
    }
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
