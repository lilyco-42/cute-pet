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
    /// 依据 manifest 的 dress/face 表选择本次要绘制的层。
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
        // z 序: 0=身体/服装 1=表情 2=头发(髪かぶせ) 3=腮红/泪/气息(组!=0 且非表情)
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

/// 数字键 1..=9 → 表情索引
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

/// 互斥播放: 播新语音前停掉上一个, 避免多个声音重叠。
/// 所有语音播放(点击/E键/TTS/兜底/预置)统一走这里。
pub(crate) fn play_voice_vol(sound: &Sound, master_volume: f32, last: &mut Option<Sound>) {
    if let Some(prev) = last.take() {
        stop_sound(&prev);
    }
    play_vol(sound, master_volume);
    *last = Some(sound.clone());
}

// ---------------- 测试 ----------------
//
// `selected_layers` 只读 `set_meta`/`dress`/`diff`, **不碰 `textures`** —— 所以
// 用空 HashMap 构造 Pet 就能在无 GPU 的 `cargo test` 里验证分层选择。
// manifest 用合成的最小样本(而不是真素材): 商业构建下角色素材不存在, 测试必须
// 两种配置都能跑。

#[cfg(test)]
mod tests {
    use super::*;

    /// 合成 manifest: 4 个 dress 行(两条同 dress 不同 diff) + 4 个 face 行
    /// (含"组不存在""没有 `/` 分隔"两个异常样本); items 用字符串键(真实格式)。
    const MANIFEST: &str = r#"{
      "stand": {"filename": "s.png", "xoffset": 0, "yoffset": 0},
      "info": {
        "dress": [
          {"dress": "school", "diff": 0, "layer": "body"},
          {"dress": "school", "diff": 1, "layer": "body_alt"},
          {"dress": "yukata", "diff": 0, "layer": "body_yukata"},
          {"dress": "twin",   "diff": 0, "layer": "shared"}
        ],
        "face": [
          {"face": "smile",   "layer": "表情/smile"},
          {"face": "dup",     "layer": "表情/shared"},
          {"face": "noslash", "layer": "shared"},
          {"face": "ghost",   "layer": "没有这个组/x"}
        ]
      },
      "composition": {
        "groups": {"表情": 1},
        "items": {
          "a": {"layer_type":0,"name":"body","left":0,"top":0,"w":10,"h":20,"opacity":255,"visible":1,"layer_id":10,"group":0},
          "b": {"layer_type":0,"name":"body_alt","left":0,"top":0,"w":10,"h":20,"opacity":255,"visible":1,"layer_id":11,"group":0},
          "c": {"layer_type":0,"name":"body_yukata","left":0,"top":0,"w":10,"h":20,"opacity":255,"visible":1,"layer_id":12,"group":0},
          "d": {"layer_type":0,"name":"smile","left":0,"top":0,"w":10,"h":20,"opacity":255,"visible":1,"layer_id":13,"group":1},
          "e": {"layer_type":0,"name":"shared","left":0,"top":0,"w":10,"h":20,"opacity":255,"visible":1,"layer_id":20,"group":0}
        }
      },
      "layer_count": 5
    }"#;

    fn pet(dress: &str, diff: u32) -> Pet {
        Pet {
            dress: dress.to_string(),
            face: String::new(),
            diff,
            textures: HashMap::new(),
            set_meta: serde_json::from_str(MANIFEST).expect("测试 manifest 必须能解析"),
            scale: 1.0,
            xoff: 0.0,
            yoff: 0.0,
        }
    }

    /// items 是 HashMap, 选中顺序不确定 —— 断言前先排序。
    fn names(layers: Vec<&LayerItem>) -> Vec<String> {
        let mut v: Vec<String> = layers.into_iter().map(|it| it.name.clone()).collect();
        v.sort();
        v
    }

    /// dress 行必须**同时**匹配 dress 名与 diff: 只比 dress 名会让改色版(c1/c2)
    /// 一起画出来, 立绘出现重影。
    #[test]
    fn dress_row_is_picked_by_dress_and_diff() {
        assert_eq!(names(pet("school", 0).selected_layers("")), vec!["body"]);
        assert_eq!(names(pet("school", 1).selected_layers("")), vec!["body_alt"]);
        assert_eq!(names(pet("yukata", 0).selected_layers("")), vec!["body_yukata"]);
    }

    /// 未知 dress 名 → 空集(调用方按"无层可画"降级, 不该 panic)。
    #[test]
    fn unknown_dress_selects_nothing() {
        assert!(pet("nope", 0).selected_layers("").is_empty());
    }

    /// face 行走 `<组名>/<层名>` 二级查找: 组名要先在 composition.groups 里查到 id。
    #[test]
    fn face_row_resolves_group_and_name() {
        assert_eq!(
            names(pet("school", 0).selected_layers("smile")),
            vec!["body", "smile"]
        );
    }

    /// 组名不在 composition.groups 里 → 静默跳过(素材缺组不该崩, 也不该画错层)。
    #[test]
    fn face_row_with_missing_group_is_skipped() {
        assert_eq!(names(pet("school", 0).selected_layers("ghost")), vec!["body"]);
    }

    /// face 行没有 `/` 分隔 → 跳过(不 panic)。
    #[test]
    fn face_row_without_slash_is_skipped() {
        assert_eq!(names(pet("school", 0).selected_layers("noslash")), vec!["body"]);
    }

    /// 同一 layer_id 被 dress 与 face 同时命中时只能出现一次 —— 重复绘制会让
    /// 带透明度的层叠加深(立绘局部变脏)。
    #[test]
    fn layer_hit_by_both_dress_and_face_is_deduplicated() {
        // dress="twin" 命中 shared(layer_id=20), face="dup" 也命中同一个 layer_id
        assert_eq!(names(pet("twin", 0).selected_layers("dup")), vec!["shared"]);
    }

    /// 数字键映射: 表情索引 0..=8 → Key1..Key9, 越界 → Key0(兜底不 panic)。
    #[test]
    fn digit_key_maps_index_to_row_number() {
        let expect = [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
            KeyCode::Key6,
            KeyCode::Key7,
            KeyCode::Key8,
            KeyCode::Key9,
        ];
        for (idx, key) in expect.iter().enumerate() {
            assert_eq!(digit_key(idx as u32), *key, "索引 {idx} 的映射键不对");
        }
        assert_eq!(digit_key(9), KeyCode::Key0);
        assert_eq!(digit_key(u32::MAX), KeyCode::Key0);
    }
}
