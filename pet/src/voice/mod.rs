//! 语音域: 台词-语音-表情映射(可编辑元数据)与播放。

use std::collections::HashMap;

use ply_engine::prelude::*;
use crate::assets::*;

// ---------------- 语音元数据(可编辑) ----------------

/// 语音 → (表情, 中文台词, 日文台词[可选]) 映射表。
/// 每条语音: (文件名, 表情face, 中文台词, 日文台词)
pub(crate) struct Pet {
    pub(crate) dress: String,
    pub(crate) face: String,
    pub(crate) diff: u32,
    pub(crate) textures: HashMap<u32, Texture2D>,
    pub(crate) set_meta: SetManifest,
    pub(crate) scale: f32,
    pub(crate) xoff: f32,
    pub(crate) yoff: f32,
}

impl Pet {
    /// 依据 manifest �?dress/face 表选择本次要绘制的层�?
    pub(crate) fn selected_layers(&self, face: &str) -> Vec<&LayerItem> {
        let items = &self.set_meta.composition.items;
        let mut out: Vec<&LayerItem> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for d in &self.set_meta.info.dress {
            if d.dress == self.dress && d.diff == self.diff {
                for it in items.values() {
                    if it.name == d.layer && seen.insert(it.layer_id) {
                        out.push(it);
                    }
                }
            }
        }
        for f in &self.set_meta.info.face {
            if f.face == face {
                if let Some((group, name)) = f.layer.split_once('/') {
                    if let Some(&gid) = self.set_meta.composition.groups.get(group) {
                        for it in items.values() {
                            if it.group == gid && it.name == name && seen.insert(it.layer_id) {
                                out.push(it);
                            }
                        }
                    }
                }
            }
        }
        out
    }

    pub(crate) fn draw(&mut self, face: &str) {
        let t = macroquad::time::get_time() as f32;
        let bob = (t * 1.2).sin() * 4.0; // 待机上下浮动

        let layers = self.selected_layers(face);
        let n_layers = layers.len();
        // z �? 0=身体/服装 1=表情 2=头发(髪かぶせ) 3=腮红/�?气息(�?=0 且非表情)
        let face_g1 = self.set_meta.composition.groups.get("表情").copied().unwrap_or(0);
        let face_g2 = self.set_meta.composition.groups.get("表情（追加）").copied().unwrap_or(0);
        let z_index = |it: &LayerItem| -> i32 {
            if it.name.contains("髪かぶせ") {
                2
            } else if it.group == face_g1 || it.group == face_g2 {
                1
            } else if it.group != 0 {
                3
            } else {
                0
            }
        };
        let mut ordered: Vec<(&LayerItem, i32)> = layers.iter().map(|&it| (it, z_index(it))).collect();
        ordered.sort_by_key(|(_, z)| *z);
        for (it, _) in ordered {
            let Some(tex) = self.textures.get(&it.layer_id) else { continue };
            let x = self.xoff + it.left as f32 * self.scale;
            let y = self.yoff + it.top as f32 * self.scale + bob;
            let w = it.w as f32 * self.scale;
            let h = it.h as f32 * self.scale;
            let alpha = (it.opacity as f32 / 255.0).min(1.0);
            draw_texture_ex(
                tex,
                x,
                y,
                MacroquadColor::new(1.0, 1.0, 1.0, alpha),
                DrawTextureParams {
                    dest_size: Some(vec2(w, h)),
                    ..Default::default()
                },
            );
        }
        if std::env::var("PET_DEBUG").is_ok() {
            draw_text(
                &format!("Murasame dress={} face={} layers={}", self.dress, face, n_layers),
                self.xoff + 4.0,
                self.yoff + 16.0,
                16.0,
                MacroquadColor::new(1.0, 1.0, 1.0, 1.0),
            );
        }
    }
}

/// 数字�?1..=9 �?表情索引
pub(crate) fn digit_key(idx: u32) -> KeyCode {
    match idx {
        0 => KeyCode::Key1,
        1 => KeyCode::Key2,
        2 => KeyCode::Key3,
        3 => KeyCode::Key4,
        4 => KeyCode::Key5,
        5 => KeyCode::Key6,
        6 => KeyCode::Key7,
        7 => KeyCode::Key8,
        8 => KeyCode::Key9,
        _ => KeyCode::Key0,
    }
}

/// 按主音量播放声音(替代 play_sound_once 实现页面内调音量)。
/// Android 桌宠页面内无法用系统音量键, miniquad 不支持, 故应用内主音量控制。
pub(crate) fn play_vol(sound: &Sound, master_volume: f32) {
    play_sound(
        sound,
        PlaySoundParams {
            looped: false,
            volume: master_volume.clamp(0.0, 1.0),
        },
    );
}

/// 互斥播放: 播新语音前停掉上一�? 避免多个声音重叠�?
/// 所有语音播�?点击/E�?TTS/兜底/预置)统一走这里�?
pub(crate) fn play_voice_vol(sound: &Sound, master_volume: f32, last: &mut Option<Sound>) {
    if let Some(prev) = last.take() {
        stop_sound(&prev);
    }
    play_vol(sound, master_volume);
    *last = Some(sound.clone());
}
