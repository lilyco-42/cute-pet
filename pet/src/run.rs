//! 丛雨(ムラサメ) 桌宠渲染层 — ply-engine / macroquad:
//! manifest 驱动图层选择 + 逐层 draw_texture_ex 合成 + 待机动画 + 表情切换 + 透明置顶窗口。
//!
//! 对外只有两个入口, 都在这里(lib 的一部分):
//! - [`run`]: 异步渲染主循环;
//! - [`start`]: 用 [`window_conf`] 装配窗口并驱动 [`run`]。
//!
//! 桌面二进制(`src/main.rs`)和鸿蒙 staticlib(`pet/host`)共用 `start()`,
//! 不再各写一份装配。此前这段整体躺在二进制 crate 的 main.rs 里, 鸿蒙壳只能
//! 用 `#[path = "../../src/main.rs"] mod app;` 把整个文件 include 进来,
//! 于是壳要复制一整份依赖树(并依赖本机 junction, 干净 clone 必然构建失败)。
use ply_engine::prelude::*;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use lazy_ply::components::{chat_panel, ChatMessage, ChatPanelEvents, ChatPanelState};

use crate::app::{self, AppEvent, AppState};
use crate::chat::{voice_meta, Lang, Persona, LANG_TOGGLE, LLM_HINT_TEXT};

// 平台模块挂在 crate 根(声明见 lib.rs), 本文件是子模块, 需显式引入。
#[cfg(target_os = "windows")]
use crate::windows;
#[cfg(target_os = "macos")]
use crate::macos;
#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
use crate::linux;

/// 免费模型渠道信息页(放 GitHub 或项目文档, 便于持续维护)。
const LLM_HINT_URL: &str = "https://github.com/lazy-plxy/cute-pet/blob/main/docs/free-llm.md";

/// W2 视觉: 聊天面板快捷问题文案(main.rs 拦截处理: 截屏让丛雨"看着你")。
const VISION_QUESTION: &str = "🔍 看看我在干嘛";
const VISION_QUESTION_JP: &str = "🔍 何を見てる？";
/// 语言切换按钮文案(中/日)
const LANG_TOGGLE_JP: &str = "🌐 言語切替";
/// LLM 免费模型提示(日)
const LLM_HINT_TEXT_JP: &str = "AI 会話: 無料モデル → NVIDIA NIM · OpenRouter · 商湯 (タップで表示)";
/// 快捷问题列表(中/日, 与 lazy-ply chat_panel 默认一致)
const QUICK_ZH: &[&str] = &[
    "在吗？",
    "吃饭了吗？",
    "想我了吗？",
    "心情不好",
    "晚安",
    "你会一直陪我吗？",
    "🌐 切换语言",
    "🔍 看看我在干嘛",
];
const QUICK_JP: &[&str] = &[
    "いる？",
    "ご飯食べた？",
    "会いたかった？",
    "機嫌悪い",
    "おやすみ",
    "ずっと一緒にいてくれる？",
    "🌐 言語切替",
    "🔍 何を見てる？",
];
const PLACEHOLDER_JP: &str = "日本語で入力…";
const SEND_JP: &str = "送信";

/// 按当前语言切换聊天面板 UI 文案(快捷问题/输入框/发送/LLM 提示)。
fn apply_ui_lang(state: &mut ChatPanelState, lang: Lang) {
    if lang == Lang::Jp {
        state.quick_questions = QUICK_JP;
        state.input_placeholder = PLACEHOLDER_JP;
        state.send_label = SEND_JP;
        if state.llm_hint.is_some() {
            state.llm_hint = Some(LLM_HINT_TEXT_JP);
        }
    } else {
        state.quick_questions = QUICK_ZH;
        state.input_placeholder = "说点什么…";
        state.send_label = "发送";
        if state.llm_hint.is_some() {
            state.llm_hint = Some(LLM_HINT_TEXT);
        }
    }
}

/// 打开 LLM 免费模型渠道说明页(桌面: 系统默认浏览器; WASM: JS 拦截 OPENURL 前缀)。
#[cfg(not(any(target_os = "android", target_env = "ohos")))]
fn open_llm_hint_page() {
    #[cfg(target_arch = "wasm32")]
    {
        // WASM: 与 SPEAK 同理, 由 build/web 的 JS 拦截 console.log 打开新标签页
        println!("OPENURL:{LLM_HINT_URL}");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = std::process::Command::new("cmd")
            .arg("/C")
            .arg("start")
            .arg("")
            .arg(LLM_HINT_URL)
            .spawn();
    }
}

/// Android/HarmonyOS 桩(系统意图打开浏览器, 简化: 无操作, 避免未用警告)。
#[cfg(any(target_os = "android", target_env = "ohos"))]
fn open_llm_hint_page() {
    // TODO: Android/鸿蒙 用系统意图打开浏览器; 暂不实现(桌面/WASM 为主)
}

// 平台模块(windows/macos/linux)是 crate 根级模块, 声明在 lib.rs —— 本文件
// 自己已经是子模块, `mod windows;` 会被解析成 src/run/windows.rs。

const MANIFEST_PATH: &str = "murasame_manifest.json";
const LAYER_DIR: &str = "murasame_layers";
// 缩放: 桌面 1/3(600x850 窗口), 移动端/鸿蒙放大填满屏幕
#[cfg(any(target_os = "android", target_env = "ohos"))]
const SCALE: f32 = 0.6;
#[cfg(not(any(target_os = "android", target_env = "ohos")))]
const SCALE: f32 = 1.0 / 3.0;

/// 跨平台资�? 编译期嵌入二进制(rust-embed), 所有平台统一, 无运行时路径问题�?
/// 第三方版权角色素材(丛雨/むらさめ)的路径前缀。
/// 版权归柚子社《千恋＊万花》、不归我方 —— 商业构建(`--no-default-features`,
/// 无 bundle-murasame)不编入二进制, 运行时改从用户素材目录加载。
const CHARACTER_ASSET_PREFIXES: [&str; 4] = [
    "murasame_manifest.json",
    "murasame_persona.txt",
    "murasame_corpus",
    "murasame_layers/",
];

/// 是否属于第三方版权角色素材(而非我方原创资产)。
pub(crate) fn is_character_asset(p: &str) -> bool {
    CHARACTER_ASSET_PREFIXES.iter().any(|s| p.starts_with(s))
}

/// 只含角色素材的嵌入表 —— 仅在 bundle-murasame(默认: 开发 / 测试构建)下编译。
#[cfg(feature = "bundle-murasame")]
#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
#[include = "murasame_*"]
struct CharAsset;

/// 核心资产(字体 / UI 预设 / 对话库等): 始终编入, 且已排除角色素材。
/// 商业构建靠这张表就够 —— 二进制里不含任何柚子社素材。
#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
#[exclude = "murasame_*"]
struct CoreAsset;

/// 用户素材目录: 角色素材不随商业二进制分发, 运行时**优先**从这里读。
/// 桌面端 = 可执行文件同级的 assets/ 目录(把素材放程序旁边即可)。
/// 用户素材目录候选(按优先级)。角色素材不随商业二进制分发, 运行时**优先**从这里读。
///
/// 1. `PET_ASSETS_DIR` 环境变量 —— 跨平台通用逃生口, 真机路径不确定时用它直接指定。
/// 2. 可执行文件同级 `assets/`(桌面)。
/// 3. 平台专属目录:
///    - Android: app 专属外部目录(无需权限, 本项目 minSdk 26 满足) + `/sdcard` 兜底
///    - HarmonyOS: el2 应用沙箱 files 目录
///    - iOS: App Documents(可通过「文件」App 放入)
///    - WASM: 浏览器没有本地文件系统, **不支持**外部目录
fn external_asset_dirs() -> Vec<std::path::PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();

        if let Some(d) = std::env::var_os("PET_ASSETS_DIR") {
            dirs.push(std::path::PathBuf::from(d));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                dirs.push(parent.join("assets"));
            }
        }

        #[cfg(target_os = "android")]
        {
            // app 专属外部目录: 无需任何权限即可读写(API 19+)
            dirs.push(std::path::PathBuf::from(
                "/sdcard/Android/data/rust.cute_pet/files/assets",
            ));
            // 兜底: 用户用文件管理器直接放 sdcard 根目录
            dirs.push(std::path::PathBuf::from("/sdcard/cute-pet/assets"));
        }

        #[cfg(target_env = "ohos")]
        {
            dirs.push(std::path::PathBuf::from(
                "/data/storage/el2/base/haps/entry/files/assets",
            ));
        }

        #[cfg(target_os = "ios")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                dirs.push(std::path::PathBuf::from(home).join("Documents").join("assets"));
            }
        }

        dirs
    }
}

fn character_asset_missing_msg(p: &str) -> String {
    let dirs = external_asset_dirs();
    if dirs.is_empty() {
        // WASM: 浏览器没有本地文件系统, 用户无法放置素材
        format!(
            "角色素材缺失: {p}\n本版本不含第三方版权角色素材(版权归柚子社《千恋＊万花》)。\n\
             当前平台(WASM)无法读取本地素材目录 —— 需要角色请使用桌面版, 并把素材放到程序同级的 assets/ 目录。"
        )
    } else {
        let list = dirs
            .iter()
            .map(|d| format!("  - {}", d.display()))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "角色素材缺失: {p}\n本版本不含第三方版权角色素材(版权归柚子社《千恋＊万花》)。\n\
             请把角色素材放到以下任一目录(也可用 PET_ASSETS_DIR 环境变量指定):\n{list}"
        )
    }
}

pub(crate) fn load_asset(p: &str) -> anyhow::Result<Vec<u8>> {
    // 1) 用户素材目录优先 —— 商业版靠这一条拿到素材(按优先级逐个尝试)
    for dir in external_asset_dirs() {
        if let Ok(b) = std::fs::read(dir.join(p)) {
            return Ok(b);
        }
    }
    // 2) 编译期嵌入
    if is_character_asset(p) {
        #[cfg(feature = "bundle-murasame")]
        if let Some(f) = CharAsset::get(p) {
            return Ok(f.data.into_owned());
        }
        anyhow::bail!("{}", character_asset_missing_msg(p))
    } else if let Some(f) = CoreAsset::get(p) {
        Ok(f.data.into_owned())
    } else {
        anyhow::bail!("资产缺失: {p}")
    }
}

/// 角色素材加载失败时打印提示并降级(不 panic)。
/// 商业版正常情况下就会走这条路径 —— 缺素材属于预期, 不是崩溃理由。
pub(crate) fn load_character_asset_or_warn(p: &str) -> Option<Vec<u8>> {
    match load_asset(p) {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("[cute-pet] {e}");
            None
        }
    }
}

// ---------------- manifest 模型 ----------------

#[derive(Deserialize, Clone)]
struct Manifest {
    character: String,
    name_cn: String,
    voice_code: String,
    sets: HashMap<String, SetManifest>,
}

#[derive(Deserialize, Clone)]
struct SetManifest {
    #[allow(dead_code)]
    stand: Stand,
    info: Info,
    composition: Composition,
    #[allow(dead_code)]
    layer_count: usize,
}

#[derive(Deserialize, Clone)]
struct Stand {
    #[allow(dead_code)]
    filename: String,
    #[allow(dead_code)]
    xoffset: i32,
    #[allow(dead_code)]
    yoffset: i32,
}

#[derive(Deserialize, Clone)]
struct Info {
    dress: Vec<DressRow>,
    face: Vec<FaceRow>,
}

#[derive(Deserialize, Clone)]
struct DressRow {
    dress: String,
    diff: u32,
    layer: String,
}

#[derive(Deserialize, Clone)]
struct FaceRow {
    face: String,
    layer: String,
}

#[derive(Deserialize, Clone)]
struct Composition {
    groups: HashMap<String, u32>,
    items: HashMap<String, LayerItem>,
}

#[derive(Deserialize, Clone)]
struct LayerItem {
    #[allow(dead_code)]
    layer_type: u32,
    name: String,
    left: u32,
    top: u32,
    w: u32,
    h: u32,
    opacity: u32,
    #[allow(dead_code)]
    visible: u32,
    layer_id: u32,
    group: u32,
}

// ---------------- 眨眼/口型动画 ----------------

/// 孪生表情映射: (基础表情, 眨眼闭眼�? 说话开口版)�?
/// �?`assets/pet/murasame` �?b/e/m 合成层像素分析生�?闭眼�?= 眼白�?)�?
/// 来源见仓库工�? 对每个基础脸选「嘴部差异最�?+ 眼部变化最大」的合成脸作眨眼,
/// 「眼部差异最�?+ 嘴部变化最大」的作说话口型�?
const FACE_TWINS: &[(&str, &str, &str)] = &[
    ("01", "30", "39"),
    ("02", "33", "39"),
    ("03", "30", "39"),
    ("04", "33", "39"),
    ("05", "33", "34"),
    ("06", "30", "38"),
    ("07", "40", "36"),
    ("08", "40", "36"),
    ("09", "40", "36"),
    ("10", "30", "35"),
    ("11", "30", "39"),
    ("12", "30", "32"),
    ("13", "40", "39"),
    ("14", "30", "39"),
    ("15", "40", "31"),
    ("16", "40", "31"),
    ("17", "37", "36"),
    ("18", "30", "39"),
    ("19", "40", "39"),
    ("20", "33", "39"),
    ("21", "29", "39"),
    ("22", "30", "39"),
    ("23", "28", "34"),
    ("24", "30", "39"),
    ("25", "40", "34"),
    ("26", "40", "35"),
];

/// 当前表情的动画孪�?眨眼/说话)。查不到则返�?None(该表情无动画素材)�?
fn face_twins(face: &str) -> Option<(&str, &str)> {
    FACE_TWINS.iter().find(|(b, _, _)| *b == face).map(|(_, bl, tk)| (*bl, *tk))
}

// ---------------- 语音元数据(可编辑) ----------------

/// 语音 → (表情, 中文台词, 日文台词[可选]) 映射表。
/// 每条语音: (文件名, 表情face, 中文台词, 日文台词)
struct Pet {
    dress: String,
    face: String,
    diff: u32,
    textures: HashMap<u32, Texture2D>,
    set_meta: SetManifest,
    scale: f32,
    xoff: f32,
    yoff: f32,
}

impl Pet {
    /// 依据 manifest �?dress/face 表选择本次要绘制的层�?
    fn selected_layers(&self, face: &str) -> Vec<&LayerItem> {
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

    fn draw(&mut self, face: &str) {
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
fn digit_key(idx: u32) -> KeyCode {
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
fn play_vol(sound: &Sound, master_volume: f32) {
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
fn play_voice_vol(sound: &Sound, master_volume: f32, last: &mut Option<Sound>) {
    if let Some(prev) = last.take() {
        stop_sound(&prev);
    }
    play_vol(sound, master_volume);
    *last = Some(sound.clone());
}

/// 右上角绘制音量指示条(音量变化后短暂显�?。y=90 避开 Android 状态栏(�?0-66px)�?
fn draw_volume_indicator(volume: f32) {
    let x = screen_width() - 70.0;
    let y = 90.0;
    let w = 46.0;
    let h = 14.0;
    draw_rectangle(x, y, w, h, MacroquadColor::new(0.0, 0.0, 0.0, 0.55));
    let fill = (w - 4.0) * volume.clamp(0.0, 1.0);
    if fill > 1.0 {
        draw_rectangle(x + 2.0, y + 2.0, fill, h - 4.0, MacroquadColor::new(0.3, 0.85, 0.5, 0.95));
    }
}

/// 角色头顶的台词气�? 半透明黑底白字 + 小三角尾巴指向角色�?
/// 字号/内边距随 ui_scale 缩放(Android 2.7x), 长文本自动换行�?
fn draw_speech_bubble(text: &str, font: &macroquad::text::Font, cx: f32, char_top: f32) {
    let ui_scale = if cfg!(any(target_os = "android", target_env = "ohos")) {
        (screen_width() / 400.0).clamp(1.0, 3.5)
    } else {
        1.0
    };
    let font_size = (20.0 * ui_scale).round() as u16;
    let pad_x = 14.0 * ui_scale;
    let pad_y = 9.0 * ui_scale;
    let max_w = (screen_width() * 0.72).max(120.0);
    // 自动换行 + 测多行尺�?行距 1.3)
    let wrapped = macroquad::text::wrap_text(text, Some(font), font_size, 1.0, max_w - pad_x * 2.0);
    let dims = macroquad::text::measure_multiline_text(&wrapped, Some(font), font_size, 1.0, Some(1.3));
    let bw = dims.width + pad_x * 2.0;
    let bh = dims.height + pad_y * 2.0;
    let bx = (cx - bw / 2.0).clamp(6.0, screen_width() - bw - 6.0);
    let by = (char_top - bh - 18.0 * ui_scale).max(4.0);
    let bg = MacroquadColor::new(0.0, 0.0, 0.0, 0.75);
    draw_rectangle(bx, by, bw, bh, bg);
    // 三角尾巴
    let tail_w = 12.0 * ui_scale;
    let tail_h = 10.0 * ui_scale;
    let tail_x = (cx - tail_w / 2.0).clamp(bx + 4.0, bx + bw - tail_w - 4.0);
    draw_triangle(
        macroquad::math::Vec2::new(tail_x, by + bh),
        macroquad::math::Vec2::new(tail_x + tail_w, by + bh),
        macroquad::math::Vec2::new(tail_x + tail_w / 2.0, by + bh + tail_h),
        bg,
    );
    // 多行白字(首行基线 = 气泡�?top + offset_y)
    macroquad::text::draw_multiline_text_ex(
        &wrapped,
        bx + pad_x,
        by + pad_y + dims.offset_y,
        Some(1.3),
        macroquad::text::TextParams {
            font: Some(font),
            font_size,
            color: MacroquadColor::new(1.0, 1.0, 1.0, 1.0),
            ..Default::default()
        },
    );
}

// ---------------- 窗口 ----------------

pub fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: "丛雨 - 桌宠雏形".to_owned(),
            window_width: 600,
            window_height: 850,
            high_dpi: true,
            window_resizable: false,
            // 关闭 MSAA: 模拟器宿�?GPU 透传(�?AMD Translator)不提�?
            // EGL_SAMPLES=1 配置, 导致 miniquad egl.rs cfg_count=0 panic
            sample_count: 0,
            platform: miniquad::conf::Platform {
                webgl_version: miniquad::conf::WebGLVersion::WebGL2,
                // 功耗: Android 桌宠常驻悬浮, 满帧率渲染空转耗电
                // → 阻塞事件循环 + 100ms 周期唤醒(空闲 ~10fps, 交互即时响应)
                #[cfg(target_os = "android")]
                blocking_event_loop: true,
                #[cfg(target_os = "android")]
                sleep_interval_ms: Some(100),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }
}

// ---------------- 入口 ----------------

/// 同步入口: 装配窗口并驱动 [`run`]。
///
/// 桌面二进制与鸿蒙 staticlib 共用, 两边不会再编出不同的窗口配置。
/// (原先是 `#[macroquad::main(window_conf)]`, 该宏只能在二进制 crate 里展开,
/// 鸿蒙壳要用它就得把整个 main.rs include 过去。)
pub fn start() {
    macroquad::Window::from_config(window_conf(), run());
}

pub async fn run() {
    // 商业构建(无 bundle-murasame)不含角色素材 → 打印提示后退出渲染循环, 不 panic。
    let Some(manifest_bytes) = load_character_asset_or_warn(MANIFEST_PATH) else {
        eprintln!("[cute-pet] 未找到角色素材, 退出渲染循环(软件其余模块化能力不受影响)。");
        return;
    };
    let mut manifest: Manifest = serde_json::from_slice(&manifest_bytes).expect("解析 manifest.json");
    println!("加载角色: {} ({}) voice={}", manifest.name_cn, manifest.character, manifest.voice_code);

    let set_meta = manifest.sets.remove("a").expect("缺少 set a");

    // 角色轮廓 bbox: 让窗口贴合立�?顶部透明区域裁掉)
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    for it in set_meta.composition.items.values() {
        min_x = min_x.min(it.left);
        min_y = min_y.min(it.top);
        max_x = max_x.max(it.left + it.w);
        max_y = max_y.max(it.top + it.h);
    }
    let bbox_w = max_x - min_x;
    let bbox_h = max_y - min_y;
    // 原始立绘尺寸(不含内边�?, 供渲染缩放使�?
    let sprite_w = bbox_w;
    let sprite_h = bbox_h;
    // 内边�?画布坐标): 窗口=舞台(顶部气泡�?+ 底部聊天控件�?+ 中部立绘),
    // 聊天 UI 不再覆盖立绘
    const PAD: u32 = 90;
    const PAD_TOP: u32 = 480;    // 顶部气泡�?
    const PAD_BOTTOM: u32 = 540; // 底部聊天控件�?
    let pad_x = PAD.min(bbox_w / 3);
    let pad_top = PAD_TOP.min(bbox_h / 2);
    let pad_bottom = PAD_BOTTOM.min(bbox_h / 2);
    min_x = min_x.saturating_sub(pad_x);
    min_y = min_y.saturating_sub(pad_top);
    max_x = max_x + pad_x;
    max_y = max_y + pad_bottom;
    let bbox_w = max_x - min_x;
    let bbox_h = max_y - min_y;
    println!("角色轮廓(+padding): x {}..{} y {}..{} ({}x{})", min_x, max_x, min_y, max_y, bbox_w, bbox_h);
    let win_w = (bbox_w as f32 * SCALE).round() as i32;
    let win_h = (bbox_h as f32 * SCALE).round() as i32;
    #[cfg(target_os = "windows")]
    macroquad::miniquad::window::set_window_size(win_w as u32, win_h as u32);

    let mut textures: HashMap<u32, Texture2D> = HashMap::new();
    for (id, item) in &set_meta.composition.items {
        if let Ok(bytes) = load_asset(&format!("{LAYER_DIR}/a_{id}.png")) {
            textures.insert(item.layer_id, Texture2D::from_file_with_format(&bytes, None));
        }
    }
    println!("已加载{} 层纹理", textures.len());

    // 语音�? �?VOICE_META 表加�? 中文克隆问�?greeting) + 日文原声反应(mur001) 分开
    let mut cn_voices: Vec<(String, Sound)> = Vec::new(); // 中文(点击/兜底�?
    let mut jp_voices: Vec<(String, Sound)> = Vec::new(); // 日文原声反应
    for (name, _face, _zh, _jp) in crate::chat::VOICE_META {
        let path = if name.starts_with("greeting_") {
            // greeting �?ogg: WASM 走浏览器 decodeAudioData, 原生支持 vorbis
            format!("voice/greeting/{name}.ogg")
        } else {
            format!("voice/{name}.ogg")
        };
        if let Ok(bytes) = load_asset(&path) {
            if let Ok(s) = load_sound_from_bytes(&bytes).await {
                if name.starts_with("greeting_") {
                    cn_voices.push((name.to_string(), s));
                    println!("问候语: {name}");
                } else {
                    jp_voices.push((name.to_string(), s));
                    println!("语音: {name}");
                }
            }
        }
    }
    println!("语音库: 中文 {} 条, 日文 {} 条", cn_voices.len(), jp_voices.len());
    // 预置对话�? 中文问答(文本独立于语�? 保证回复一定中�?
    let mut preset_kws: Vec<String> = Vec::new();   // 问题(用于输入匹配)
    let mut preset_answers: Vec<String> = Vec::new(); // 回答文本(中文)
    let mut preset_sounds: Vec<Sound> = Vec::new(); // 对应语音(可能为空)
    let dialog_raw: Option<String> = {
        #[cfg(target_arch = "wasm32")]
        {
            load_asset("dialog_preset.txt").ok().map(|b| String::from_utf8_lossy(&b).into_owned())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // 桌面: PET_VOICE_DIR 指向外部语音目录时用�?dialog.txt; 否则(�?Android)用内嵌预置问�?
            std::env::var("PET_VOICE_DIR")
                .ok()
                .and_then(|d| std::fs::read_to_string(std::path::Path::new(&d).join("dialog.txt")).ok())
                .or_else(|| load_asset("dialog_preset.txt").ok().map(|b| String::from_utf8_lossy(&b).into_owned()))
        }
    };
    if let Some(raw) = &dialog_raw {
        for line in raw.lines() {
            if let Some((q, a)) = line.split_once('|') {
                preset_kws.push(q.trim().to_string());
                preset_answers.push(a.trim().to_string());
            }
        }
    }
    // 语音: 与问答一一对应(dialog_preset 75 �?= voice_preset/fei00-74)�?
    // �?Vec<Option<Sound>> �?idx 对齐: 某条加载失败时占�?None(播放时跳�?,
    // 保证语音与文本永远不错位�?
    let mut preset_sounds: Vec<Option<Sound>> = Vec::new();
    // 内嵌 voice_preset �?idx 对齐加载
    #[cfg(target_arch = "wasm32")]
    {
        for i in 0..preset_kws.len() {
            let mut s = None;
            if let Ok(bytes) = load_asset(&format!("voice_preset/fei{i:02}.ogg")) {
                if let Ok(sound) = load_sound_from_bytes(&bytes).await {
                    s = Some(sound);
                }
            }
            preset_sounds.push(s);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(vdir) = std::env::var("PET_VOICE_DIR") {
        let map_path = std::path::Path::new(&vdir).join("map.json");
        if let Ok(raw) = std::fs::read_to_string(&map_path) {
            if let Ok(entries) = serde_json::from_str::<Vec<Vec<String>>>(&raw) {
                for i in 0..preset_kws.len() {
                    let mut s = None;
                    if let Some(e) = entries.get(i).filter(|e| e.len() >= 2) {
                        if let Ok(bytes) = std::fs::read(std::path::Path::new(&vdir).join(&e[1])) {
                            if let Ok(sound) = load_sound_from_bytes(&bytes).await {
                                s = Some(sound);
                            }
                        }
                    }
                    preset_sounds.push(s);
                }
            }
        }
    } else {
        for i in 0..preset_kws.len() {
            let mut s = None;
            if let Ok(bytes) = load_asset(&format!("voice_preset/fei{i:02}.ogg")) {
                if let Ok(sound) = load_sound_from_bytes(&bytes).await {
                    s = Some(sound);
                }
            }
            preset_sounds.push(s);
        }
    }
    println!("预置对话库: 问答 {} 条, 语音 {} 条", preset_answers.len(), preset_sounds.iter().filter(|s| s.is_some()).count());
    let mut voice_idx = 0usize;

    // 聊天�? 丛雨 persona(LLM env 门控 + 语料兜底) + CJK 字体
    // 双语: 中文语料过滤掉含假名的垃圾日语, 日文语料原样。语言用 PET_LANG / 运行时切换。
    use crate::chat::{filter_zh_corpus, Lang};
    // 语料缺失可安全降级: 空语料仍能构建 persona(只是检索无命中), 不是崩溃理由。
    let corpus_zh = load_character_asset_or_warn("murasame_corpus_zh.jsonl").unwrap_or_default();
    let corpus_jp = load_character_asset_or_warn("murasame_corpus.jsonl").unwrap_or_default();
    let mut persona_zh = Persona::murasame_from_corpus_content(
        &filter_zh_corpus(&String::from_utf8_lossy(&corpus_zh)),
    ).expect("解析中文语料失败");
    let mut persona_jp = Persona::murasame_from_corpus_content(
        &String::from_utf8_lossy(&corpus_jp),
    ).expect("解析日文语料失败");
    persona_zh.set_language(Lang::Zh);
    persona_jp.set_language(Lang::Jp);
    let mut lang: Lang = if std::env::var("PET_LANG").as_deref() == Ok("jp") { Lang::Jp } else { Lang::Zh };
    let font_bytes = load_asset("font_wenkai.ttf").expect("读取字体失败");
    // 台词气泡用字�?独立 Font 实例, �?ply 同字, macroquad draw_text_ex 直绘)
    let mq_font = macroquad::text::load_ttf_font_from_bytes(&font_bytes).ok();
    let font_data: &'static [u8] = Box::leak(font_bytes.into_boxed_slice());
    let font_asset: &'static FontAsset = Box::leak(Box::new(FontAsset::Bytes {
        file_name: "font_wenkai.ttf",
        data: font_data,
    }));
    let mut ply = Ply::<()>::new(font_asset).await;
    // 聊天面板(lazy-ply 组件): 气泡历史 + 快捷问题 + 输入�?
    let mut chat_state = ChatPanelState::default();
    // 未配�?LLM Key �?面板底部提示免费模型渠道(NVIDIA NIM / OpenRouter / 商汤)
    // 必须先设 llm_hint 再 apply_ui_lang: apply_ui_lang 会按语言挑日文/中文文案
    chat_state.llm_hint = if std::env::var("PET_LLM_API_KEY").is_ok() {
        None
    } else {
        Some("AI 对话: 免费模型 → NVIDIA NIM · OpenRouter · 商汤 (点击查看)")
    };
    // 初始语言(PET_LANG=jp 时面板也直接日文, 含 llm_hint 日文化)
    apply_ui_lang(&mut chat_state, lang);
    let chat_events: Rc<RefCell<ChatPanelEvents>> = Rc::new(RefCell::new(ChatPanelEvents::default()));
    let mut pending_voice: Option<String> = None;
    // 远程 TTS 合成(丛雨克隆音色): 后台线程�?wav, 帧循环播放�?
    // 结果携带成败: Ok(wav) 播放克隆音色; Err 立即播放内嵌兜底语音(保证点击/回复必有声音)
    let tts_result: Arc<Mutex<Option<Result<Vec<u8>, String>>>> = Arc::new(Mutex::new(None));
    // 台词气泡: (文本, 显示到此�?, 点击桌宠说话时显示在角色�?
    let mut speech_line: Option<(String, f32)> = None;
    // 互斥播放: 记录正在/刚播放的声音, 新播放前停掉�?防重�?
    let mut last_sound: Option<Sound> = None;
    // 最近一次回复文本: TTS 失败兜底时气泡显示它, (在读哪句可见)
    let mut last_reply: String = String::new();
    // W2 视觉: 按钮触发等待新截屏帧 + 分析限流 + 结果通道
    let mut vision_pending = false;
    let mut vision_last = -90.0f32;
    let (vision_tx, vision_rx) = std::sync::mpsc::channel::<String>();
    let vlm_cfg = load_vlm_config();
    // 配好 vlm_config.json(有 api_key)后, 启动即自动申请一次"看着你"(首个授权)
    let mut vision_auto_done = false;
    // 聊天键盘: 输入框焦点状态(焦点变化时唤起/收起系统 IME)
    let mut kb_shown = false;
    // [临时诊断 kb7] 指针是否曾进入输入框(边沿触发手动聚焦)
    let mut was_over_input = false;
    // F3 调试面板: 显示语音库加载数(诊断�?
    let mut debug_info = false;
    let preset_total = preset_sounds.len();
    let preset_loaded = preset_sounds.iter().filter(|s| s.is_some()).count();
    // TTS 兜底截止时刻: 提交回复时置 now+10, 超时未出结果则播内嵌语音
    let mut tts_deadline: f32 = 0.0;
    // 角色矩形(每帧更新): (x, y, w, h), 台词气泡/点击判定共用
    let mut char_rect: (f32, f32, f32, f32) = (0.0, 0.0, 0.0, 0.0);

    let mut pet = Pet {
        dress: "私服".to_string(),
        face: "01".to_string(),
        diff: 1,
        textures,
        set_meta,
        scale: SCALE,
        xoff: 0.0,
        yoff: 0.0,
    };

    let faces = ["01", "03", "04", "13", "14", "19", "21", "02", "20"];
    let verify = std::env::var("PET_VERIFY").is_ok();
    let mut frame = 0u32;

    // 透明置顶窗口(按平台调用平台层)
    #[cfg(target_os = "windows")]
    let hwnd: *mut std::ffi::c_void = macroquad::miniquad::window::windows_hwnd();
    #[cfg(target_os = "windows")]
    windows::make_transparent_pet_window(hwnd);
    #[cfg(target_os = "macos")]
    macos::make_transparent_pet_window();
    #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
    linux::make_transparent_pet_window("丛雨 - 桌宠雏形");

    // 拖拽移动
    let mut dragging = false;
    let mut grab_mx = 0.0f32;
    let mut grab_my = 0.0f32;
    // 点击互动: 说完随机台词 + 切表�?
    let mut click_since = 0.0f32;

    // 眨眼/口型动画: 眨眼计时 + 说话口型计时
    let mut blink_cycle = 0.0f32;
    let mut blink_phase = 0.0f32;   // 眨眼动画剩余时间(0 = 睁眼)
    let mut talk_until = 0.0f32;    // 口型动画持续到此�?发声时触�?

    // 主音�?0.0~1.0): 桌宠页面内可�?桌面 =/= �? Android 触摸屏两�?
    let mut master_volume: f32 = 1.0;
    let mut vol_show_until = 0.0f32;   // 音量指示条显示到此刻

    loop {
        let now = macroquad::time::get_time() as f32;
        // 透明背景: alpha=0, �?DWM 合成到桌�?
        clear_background(MacroquadColor::new(0.0, 0.0, 0.0, 0.0));
        // Android/鸿蒙 和风背景(不透明窗口, lazy-ply 组件); 桌面透明窗口不画
        #[cfg(any(target_os = "android", target_env = "ohos"))]
        lazy_ply::components::pet_background(now, screen_width(), screen_height());

        // 主音量调�?桌宠页面�?: 桌面 `-`/`=` �?`[`/`]`, Android 触摸屏两�?
        let mut vol_changed = false;
        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::LeftBracket) {
            master_volume = (master_volume - 0.1).clamp(0.0, 1.0);
            vol_changed = true;
        }
        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::RightBracket) {
            master_volume = (master_volume + 0.1).clamp(0.0, 1.0);
            vol_changed = true;
        }
        if vol_changed {
            vol_show_until = now + 1.5;
            eprintln!("[vol] {:.0}%", master_volume * 100.0);
        }

        // Android 触摸�?macroquad 映射为鼠标事�?单指=鼠标), 用鼠标释放统一处理:
        // 上半屏左右边�?= 音量; 角色范围�?= 说话。不�?touches() �?Started 判断
        // (adb/真机 DOWN+UP 可能同帧到达, Started 阶段会丢�?�?
        if is_mouse_button_released(MouseButton::Left) && now - click_since > 0.4 {
            click_since = now;
            let (mx, my) = mouse_position();
            #[cfg(any(target_os = "android", target_env = "ohos"))]
            {
                // 上半屏左右边�?�?音量
                let edge = screen_width() * 0.04;
                if my < screen_height() * 0.5 && (mx < edge || mx > screen_width() - edge) {
                    if mx < edge {
                        master_volume = (master_volume - 0.1).clamp(0.0, 1.0);
                    } else {
                        master_volume = (master_volume + 0.1).clamp(0.0, 1.0);
                    }
                    vol_show_until = now + 1.5;
                    eprintln!("[vol] {:.0}%", master_volume * 100.0);
                } else {
                    // 点击立绘中部(避开顶部气泡区与底部聊天控件�? 且横向在角色范围�?
                    let ui_scale_click = (screen_width() / 400.0).clamp(1.0, 3.5);
                    let in_ui_band = my < 150.0 * ui_scale_click + 10.0 || my > screen_height() - 190.0 * ui_scale_click - 10.0;
                    let in_char_x = mx >= char_rect.0 - 30.0 && mx <= char_rect.0 + char_rect.2 + 30.0;
                    if !in_ui_band && in_char_x {
                        // 中文模式: 克隆问�?greeting); 日文模式: 原声反应(mur001)
                        let active: &[(String, Sound)] = if lang == Lang::Zh { &cn_voices } else { &jp_voices };
                        if !active.is_empty() {
                            let (name, sound) = &active[voice_idx % active.len()];
                            play_voice_vol(sound, master_volume, &mut last_sound);
                            voice_idx += 1;
                            talk_until = now + 2.0; // 发声时触发口型动�?
                            if let Some((_, face, zh, jp)) = voice_meta(name) {
                                pet.face = (*face).to_string();
                                let text = if lang == Lang::Jp { jp.unwrap_or(zh) } else { zh };
                                speech_line = Some((text.to_string(), now + 4.0));
                            }
                        }
                    }
                }
            }
            #[cfg(not(any(target_os = "android", target_env = "ohos")))]
            {
                // 点击立绘中部(避开顶部气泡区与底部聊天控件�? �?轮播台词 + 切表�?
                let ui_scale_click = 1.0;
                let in_ui_band = my < 150.0 * ui_scale_click + 10.0 || my > screen_height() - 190.0 * ui_scale_click - 10.0;
                let in_char_x = mx >= char_rect.0 - 30.0 && mx <= char_rect.0 + char_rect.2 + 30.0;
                if !in_ui_band && in_char_x {
                    let active: &[(String, Sound)] = if lang == Lang::Zh { &cn_voices } else { &jp_voices };
                    if !active.is_empty() {
                        let (name, sound) = &active[voice_idx % active.len()];
                        play_voice_vol(sound, master_volume, &mut last_sound);
                        voice_idx += 1;
                        talk_until = now + 2.0;
                        if let Some((_, face, zh, jp)) = voice_meta(name) {
                            pet.face = (*face).to_string();
                            let text = if lang == Lang::Jp { jp.unwrap_or(zh) } else { zh };
                            speech_line = Some((text.to_string(), now + 4.0));
                        }
                    }
                }
            }
        }

        // 拖拽移动窗口
        if is_mouse_button_pressed(MouseButton::Left) {
            dragging = true;
            let (mx, my) = mouse_position();
            grab_mx = mx;
            grab_my = my;
        }
        if dragging {
            let (mx, my) = mouse_position();
            #[cfg(target_os = "windows")]
            {
                let (wx, wy) = macroquad::miniquad::window::get_window_position();
                windows::move_window(hwnd, wx as i32 + (mx - grab_mx) as i32, wy as i32 + (my - grab_my) as i32);
            }
        }
        if is_mouse_button_released(MouseButton::Left) {
            dragging = false;
        }

        for (i, f) in faces.iter().enumerate() {
            if is_key_pressed(digit_key(i as u32)) {
                pet.face = f.to_string();
            }
        }
        if is_key_pressed(KeyCode::D) {
            pet.dress = if pet.dress == "私服" { "洋装".to_string() } else { "私服".to_string() };
        }
        if is_key_pressed(KeyCode::Space) {
            pet.diff = if pet.diff == 1 { 2 } else { 1 };
        }
        // L: 切换中文/日文
        if is_key_pressed(KeyCode::L) {
            lang = if lang == Lang::Zh { Lang::Jp } else { Lang::Zh };
            apply_ui_lang(&mut chat_state, lang);
            let msg = if lang == Lang::Jp {
                "(日本語モードに切り替えたよ)".to_string()
            } else {
                "(已切换为中文模式)".to_string()
            };
            chat_state.history.push(ChatMessage::pet(&msg));
        }
        if is_key_pressed(KeyCode::F2) {
            macroquad::texture::get_screen_data().export_png("pet_screenshot.png");
            println!("screenshot saved: pet_screenshot.png");
        }
        if is_key_pressed(KeyCode::F3) {
            debug_info = !debug_info;
        }
        // E: 快速说一�?轮播, 按当前语言); Enter: 聊天输入
        if is_key_pressed(KeyCode::E) {
            let active: &[(String, Sound)] = if lang == Lang::Zh { &cn_voices } else { &jp_voices };
            if !active.is_empty() {
                let (name, sound) = &active[voice_idx % active.len()];
                play_voice_vol(sound, master_volume, &mut last_sound);
                voice_idx += 1;
                talk_until = now + 2.0;
                if let Some((_, face, zh, jp)) = voice_meta(name) {
                    pet.face = (*face).to_string();
                    let text = if lang == Lang::Jp { jp.unwrap_or(zh) } else { zh };
                    speech_line = Some((text.to_string(), now + 4.0));
                }
            }
        }
        // (聊天输入已交�?lazy-ply chat_panel: 事件�?UI 帧内收集, 回复处理在立绘绘制后)
        if is_key_pressed(KeyCode::Escape) {
            break;
        }

        // 播放回复语音(ffmpeg m4a→wav �?加载 �?播放; 仅桌�?�?Android/鸿蒙�?ffmpeg, �?TTS/兜底)
        if let Some(v) = pending_voice.take() {
            #[cfg(not(any(target_os = "android", target_env = "ohos")))]
            {
                let src = format!("../assets/pet/murasame/corpus/voice/{v}.ogg");
                if let Ok(out) = std::process::Command::new("ffmpeg")
                    .args(["-y", "-loglevel", "error", "-i", &src, "-f", "wav", "pipe:1"])
                    .output()
                {
                    if !out.stdout.is_empty() {
                        if let Ok(s) = load_sound_from_bytes(&out.stdout).await {
                            play_voice_vol(&s, master_volume, &mut last_sound);
                            talk_until = now + 2.0; // 发声时触发口型动�?
                        }
                    }
                }
            }
            #[cfg(any(target_os = "android", target_env = "ohos"))]
            let _ = v;
        }
        // 播放远程 TTS 合成的丛雨克隆音�? 失败立即播放内嵌兜底语音(点击/回复必有声音)
        if let Some(result) = tts_result.lock().unwrap().take() {
            match result {
                Ok(wav) => {
                    if let Ok(s) = load_sound_from_bytes(&wav).await {
                        play_voice_vol(&s, master_volume, &mut last_sound);
                        talk_until = now + 2.0; // 发声时触发口型动�?
                        println!("[tts] 播放克隆音色 {} KB", wav.len() / 1024);
                    }
                }
                Err(e) => {
                    eprintln!("[tts] 合成失败, 播放兜底语音: {e}");
                    // 兜底语音按当前语言选库: 日文模式播原声(mur001), 中文模式播问候(greeting)
                    let active: &[(String, Sound)] = if lang == Lang::Jp { &jp_voices } else { &cn_voices };
                    if !active.is_empty() {
                        let (name, sound) = &active[voice_idx % active.len()];
                        play_voice_vol(sound, master_volume, &mut last_sound);
                        voice_idx += 1;
                        talk_until = now + 2.0;
                        if let Some((_, face, zh, jp)) = voice_meta(name) {
                            pet.face = (*face).to_string();
                            let line = if lang == Lang::Jp { jp.unwrap_or(zh) } else { zh };
                            speech_line = Some((line.to_string(), now + 3.0));
                        }
                    }
                }
            }
        }
        // TTS 超时(网络卡死)兜底: 提交回复�?tts_deadline 内无结果 �?播内嵌语�?
        if tts_deadline > 0.0 && now > tts_deadline {
            tts_deadline = 0.0;
            // 兜底语音按当前语言选库(日文模式播原声 mur001)
            let active: &[(String, Sound)] = if lang == Lang::Jp { &jp_voices } else { &cn_voices };
            if !active.is_empty() {
                let (name, sound) = &active[voice_idx % active.len()];
                play_voice_vol(sound, master_volume, &mut last_sound);
                voice_idx += 1;
                talk_until = now + 2.0;
                if let Some((_, face, zh, jp)) = voice_meta(name) {
                    pet.face = (*face).to_string();
                    let line = if lang == Lang::Jp { jp.unwrap_or(zh) } else { zh };
                    speech_line = Some((line.to_string(), now + 3.0));
                }
            }
        }

        // 眨眼/口型动画: 计算本次绘制用表�?
        //   - 说话�?now < talk_until): 缓慢交替 基础�?�?说话口型�?�?.5Hz,
        //     说话 twin 已统一为睁眼版(39), 避免说话时眼睛高频变化像疯狂眨眼)
        //   - 空闲: �?2.6~4.2s 眨眼一�?人类眨眼频率 �?~5s, 闭眼�?120ms)
        let mut draw_face = pet.face.clone();
        if let Some((blink_twin, talk_twin)) = face_twins(&pet.face) {
            if now < talk_until {
                let phase = ((now * 2.5) as i32) & 1;
                draw_face = if phase == 1 {
                    talk_twin.to_string()
                } else {
                    pet.face.clone()
                };
            } else {
                blink_cycle += macroquad::time::get_frame_time();
                if blink_phase > 0.0 {
                    blink_phase -= macroquad::time::get_frame_time();
                    draw_face = blink_twin.to_string();
                } else {
                    // 人类眨眼频率: 3.4s ± 0.8s 平滑变化(2.6~4.2s)
                    let blink_interval = 3.4 + ((now * 0.6).sin() + 1.0) * 0.4;
                    if blink_cycle >= blink_interval {
                        blink_phase = 0.12;
                        blink_cycle = 0.0;
                    }
                }
            }
        }

        // 角色定位(每帧按当前屏幕尺寸计�?�?桌面=窗口, Android/WASM=屏幕, 自适应)
        const UI_TOP_PX: f32 = 150.0;   // 气泡区高�?
        const UI_BOTTOM_PX: f32 = 190.0; // 控件区高�?按钮+输入�?
        {
            let scr_w = screen_width();
            let scr_h = screen_height();
            let avail_h = (scr_h - UI_TOP_PX - UI_BOTTOM_PX).max(200.0);
            let render_scale = (avail_h / (sprite_h as f32 * SCALE)).min(1.0) * 0.90;
            let char_w = sprite_w as f32 * SCALE * render_scale;
            let char_h = sprite_h as f32 * SCALE * render_scale;
            let char_x = (scr_w - char_w) / 2.0;
            // 经验校准: 立绘实际渲染�?bbox 计算低约 6% 高度, 按比例上�?
            // 0.62: 角色中心略偏�? 靠近底部控件�?构图平衡)
            let char_y = UI_TOP_PX + (avail_h - char_h) * 0.62 - 0.06 * char_h;
            if frame == 0 || frame == 60 {
                println!("角色定位[帧{}]: 缩放 {} 角色 {}x{} 屏幕 {}x{}", frame, render_scale, char_w as i32, char_h as i32, scr_w as i32, scr_h as i32);
            }
            pet.scale = SCALE * render_scale;
            pet.xoff = char_x - (min_x as f32 + pad_x as f32) * SCALE * render_scale;
            pet.yoff = char_y - (min_y as f32 + pad_top as f32) * SCALE * render_scale;
            char_rect = (char_x, char_y, char_w, char_h);
        }
        pet.draw(&draw_face);

        // 台词气泡: 点击桌宠说话的台�? 显示在角色头顶上�?
        if let Some((text, until)) = speech_line.take() {
            if now < until {
                if let Some(font) = &mq_font {
                    draw_speech_bubble(&text, font, char_rect.0 + char_rect.2 / 2.0, char_rect.1);
                }
                speech_line = Some((text, until));
            }
        }

        // 聊天面板(lazy-ply 组件): 气泡历史 + 快捷问题 + 输入�? 覆盖在立绘上�?
        #[cfg(any(target_os = "android", target_env = "ohos"))]
        {
            // Android/HarmonyOS: miniquad dpi_scale 可能返回 1.0(按物理像素渲�?,
            // 用设计宽�?400dp 推算 UI 缩放, 统一放大按钮/输入�?气泡文字到可触控尺寸
            use lazy_ply::components::config::{Attrs, ButtonConfig, ButtonStateConfig, ChatPanelConfig, Style, TextFieldConfig};
            let ui_scale = (screen_width() / 400.0).clamp(1.0, 3.5);
            let mut ui = ply.begin();
            // 键盘避让: 压缩布局高度让聊天面板上�?鸿蒙键盘弹出�?
            #[cfg(target_env = "ohos")]
            {
                let kh = keyboard_height();
                if kh > 0.0 && kh < screen_height() * 0.7 {
                    ui.set_layout_dimensions(ply_engine::math::Dimensions::new(screen_width(), screen_height() - kh));
                }
            }
            let _g = Style::with(
                Attrs {
                    chat_panel: Some(ChatPanelConfig {
                        quick_columns: Some(3),
                        bubble_font_size: Some((18.0 * ui_scale) as u16),
                        ..Default::default()
                    }),
                    button: Some(ButtonConfig {
                        height: Some(48.0 * ui_scale),
                        font_size: Some((17.0 * ui_scale) as u16),
                        radius: Some(24.0 * ui_scale),
                        pad_x: Some(16.0 * ui_scale),
                        text: Some(ButtonStateConfig {
                            background: Some(0xEADDFF),
                            hover: Some(0xD6C6F5),
                            pressed: Some(0xC8B4F0),
                            foreground: Some(0x21005D),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    text_field: Some(TextFieldConfig {
                        height: Some(60.0 * ui_scale),
                        font_size: Some((18.0 * ui_scale) as u16),
                        radius: Some(18.0 * ui_scale),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                || {
                    chat_panel(&mut ui, &chat_state, &chat_events);
                },
            );
            ui.show(|_| {}).await;
            // [临时诊断 kb7] 每帧追加: 指针命中列表 + 输入框命中 + 焦点(追加模式, 便于看点击历史)
            let kb_want = Id::from(chat_state.input_id);
            let kb_focused = ui
                .focused_element()
                .map(|id| id == kb_want)
                .unwrap_or(false);
            let kb_over_input = ui.pointer_over(kb_want);
            // 引擎自动聚焦失效时的兜底: 指针进入输入框 → 手动聚焦
            if kb_over_input && !was_over_input {
                ui.set_focus(chat_state.input_id);
            }
            was_over_input = kb_over_input;
            if let Some(dir) = macroquad::miniquad::window::files_dir() {
                let kb_over_ids: Vec<String> = ui
                    .pointer_over_ids()
                    .iter()
                    .map(|id| format!("{:?}", id))
                    .collect();
                let kb_focus_raw = ui
                    .focused_element()
                    .map(|id| format!("{:?}", id))
                    .unwrap_or_else(|| "None".to_string());
                let kb_line = format!(
                    "over_input={} focus={} raw={} over={:?}\n",
                    kb_over_input, kb_focused, kb_focus_raw, kb_over_ids
                );
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(std::path::Path::new(&dir).join("kb_debug.log"))
                {
                    let _ = f.write_all(kb_line.as_bytes());
                }
            }
            // 聊天键盘: 输入框聚焦 → 唤起系统 IME(悬浮窗模式)
            if kb_focused != kb_shown {
                kb_shown = kb_focused;
                macroquad::miniquad::window::show_keyboard(kb_focused);
            }
        }
        #[cfg(not(any(target_os = "android", target_env = "ohos")))]
        {
            // 桌面/WASM: 浅色和风背景上的高对比样�?深紫按钮/白底输入�?不透明气泡)
            use lazy_ply::components::config::{Attrs, ButtonConfig, ButtonStateConfig, ChatPanelConfig, Style, TextFieldConfig};
            let mut ui = ply.begin();
            let _g = Style::with(
                Attrs {
                    chat_panel: Some(ChatPanelConfig {
                        // 注意: 不设 background �?面板是全屏容�? 不透明背景会盖住立�?
                        bubble_font_size: Some(18),
                        user_background: Some(0x6D4A8A),
                        user_foreground: Some(0xFFFFFF),
                        pet_background: Some(0xFFFFFF),
                        pet_foreground: Some(0x2A2A3E),
                        quick_columns: Some(3),
                        ..Default::default()
                    }),
                    button: Some(ButtonConfig {
                        height: Some(44.0),
                        font_size: Some(17),
                        radius: Some(22.0),
                        pad_x: Some(14.0),
                        text: Some(ButtonStateConfig {
                            background: Some(0x6D4A8A),
                            hover: Some(0x5C3D75),
                            pressed: Some(0x4E3265),
                            foreground: Some(0xFFFFFF),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    text_field: Some(TextFieldConfig {
                        height: Some(44.0),
                        font_size: Some(17),
                        radius: Some(14.0),
                        background: Some(0xFFFFFF),
                        text_color: Some(0x2A2A3E),
                        placeholder_color: Some(0x9A8FB0),
                        border: Some(0xB48AE8),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                || {
                    chat_panel(&mut ui, &chat_state, &chat_events);
                },
            );
            ui.show(|_| {}).await;
        }
        // 音量指示条画在聊天面板之�?Android 上不被控件盖�?
        if now < vol_show_until {
            draw_volume_indicator(master_volume);
        }
        // F3 调试面板: 语音库加载数(诊断"没声�?不读"�?
        if debug_info {
            let info = format!(
                "preset {}/{}\ncn {}\njp {}",
                preset_loaded, preset_total, cn_voices.len(), jp_voices.len()
            );
            if let Some(font) = &mq_font {
                draw_text_ex(
                    &info,
                    10.0,
                    130.0,
                    macroquad::text::TextParams {
                        font: Some(font),
                        font_size: 30,
                        color: MacroquadColor::new(0.0, 0.0, 0.0, 1.0),
                        ..Default::default()
                    },
                );
            }
        }
        // 统一处理输入: 聊天面板事件(快捷按钮/输入�? �?回复(气泡在下一帧显�?
        let submitted: Vec<String> = std::mem::take(&mut chat_events.borrow_mut().submitted);
        for input in submitted {
            if input.trim().is_empty() {
                continue;
            }
            // 语言切换快捷按钮(中/日文案都识别)
            if input == LANG_TOGGLE || input == LANG_TOGGLE_JP {
                lang = if lang == Lang::Zh { Lang::Jp } else { Lang::Zh };
                apply_ui_lang(&mut chat_state, lang);
                let msg = if lang == Lang::Jp {
                    "(日本語モードに切り替えたよ)".to_string()
                } else {
                    "(已切换为中文模式)".to_string()
                };
                chat_state.history.push(ChatMessage::pet(&msg));
                continue;
            }
            // LLM 免费模型提示: 点击在浏览器打开信息�? (中/日文案)
            if input == LLM_HINT_TEXT || input == LLM_HINT_TEXT_JP {
                open_llm_hint_page();
                continue;
            }
            // W2 视觉: "看看我在干嘛" → 触发 MediaProjection 截屏(Android/鸿蒙)
            if input == VISION_QUESTION || input == VISION_QUESTION_JP {
                #[cfg(any(target_os = "android", target_env = "ohos"))]
                {
                    macroquad::miniquad::window::request_screen_capture();
                    vision_pending = true;
                    let say = if lang == Lang::Jp {
                        "吾輩は君の画面を見ている…"
                    } else {
                        "吾辈正在看你的屏幕…"
                    };
                    chat_state.history.push(ChatMessage::pet(say));
                    speech_line = Some((say.to_string(), now + 3.0));
                }
                #[cfg(not(any(target_os = "android", target_env = "ohos")))]
                {
                    let say = if lang == Lang::Jp {
                        "デスクトップではまだ画面を見られないよ、Androidで会おう"
                    } else {
                        "桌面端还不能看屏幕哦，去安卓上让吾辈看着你吧"
                    };
                    chat_state.history.push(ChatMessage::pet(say));
                }
                continue;
            }
            chat_state.history.push(ChatMessage::user(&input));
            // 预置问答: 统一匹配逻辑(否定排除 + 更长关键词优�?+ 忽略标点, �?chat::preset_match)
            // 仅中文模式走预置问答(预设问答/预置语音均为中文克隆, 日语模式命中会中文回应,
            // 与所选语言不一致, issue#3)。日语模式跳过, 走下方日语 persona(回复日语)。
            let preset_hit = if lang == Lang::Zh {
                crate::chat::preset_match(&preset_kws, &input)
            } else {
                None
            };
            if let Some(idx) = preset_hit {
                let ans = preset_answers[idx].clone();
                chat_state.history.push(ChatMessage::pet(&ans));
                last_reply = ans.clone();
                // WASM: 端侧 Web Speech API 朗读回复(JS 拦截 console.log SPEAK 前缀)
                #[cfg(target_arch = "wasm32")]
                println!("SPEAK:{}", last_reply);
                if let Some(Some(s)) = preset_sounds.get(idx) {
                    play_voice_vol(s, master_volume, &mut last_sound);
                    talk_until = now + 2.0;
                    // 角色头顶气泡同步显示回复内容
                    speech_line = Some((ans.clone(), now + 3.5));
                } else {
                    // 无预置语�?�?远程 TTS(失败/超时帧循环播兜底)
                    tts_deadline = now + 10.0;
                    let target = tts_result.clone();
                    let tts_text = ans.clone();
                    std::thread::spawn(move || {
                        let result = crate::chat::synthesize_remote(&tts_text).map_err(|e| e.to_string());
                        *target.lock().unwrap() = Some(result);
                    });
                }
                continue;
            }
            // persona 回复: 按当前语言�?persona(中文语料已过滤假�?
            let persona = if lang == Lang::Zh { &persona_zh } else { &persona_jp };
            // 多轮上下�? 取最�?10 �?用户/丛雨, 旧→�?, �?LLM 前言搭后�?
            let history: Vec<(String, String)> = chat_state
                .history
                .iter()
                .rev()
                .take(10)
                .map(|m| {
                    let who = if m.from_user { "user" } else { &persona.name };
                    (who.to_string(), m.text.clone())
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            let (text, voice) = match persona.respond_llm(&history, &input) {
                Ok(s) => (s, None),
                Err(_) => match persona.respond_corpus(&input) {
                    Some((t, v)) => {
                        let t = if lang == Lang::Zh {
                            match crate::chat::translate_to_chinese(&t) {
                                Some(zh) => zh,
                                None => t,
                            }
                        } else {
                            t
                        };
                        (t, Some(v))
                    }
                    None => (
                        if lang == Lang::Jp { "(返答なし)".to_string() } else { "(无回复)".to_string() },
                        None,
                    ),
                },
            };
            chat_state.history.push(ChatMessage::pet(&text));
            last_reply = text.clone();
            // 语料语音: 桌面 ffmpeg 播放; Android/鸿蒙�?ffmpeg 丢弃�?TTS
            let mut want_tts = true;
            if let Some(v) = voice {
                #[cfg(not(any(target_os = "android", target_env = "ohos")))]
                {
                    pending_voice = Some(v);
                    want_tts = false; // 桌面已有语料原声, 不再重复合成
                }
                #[cfg(any(target_os = "android", target_env = "ohos"))]
                let _ = v;
            }
            if want_tts {
                // 远程 TTS 合成丛雨克隆音色(后台线程, 结果带成�?�?失败帧循环播兜底)
                tts_deadline = now + 10.0;
                let target = tts_result.clone();
                let tts_text = text.clone();
                std::thread::spawn(move || {
                    let result = crate::chat::synthesize_remote(&tts_text).map_err(|e| e.to_string());
                    *target.lock().unwrap() = Some(result);
                });
            }
        }

        // 验证模式: 渲染 2 秒后截图并退�?
        // W2 视觉: 配置好视觉模型后启动自动申请一次截屏授权(连续"看着你")
        #[cfg(target_os = "android")]
        if !vision_auto_done && now > 1.0 && !vlm_cfg.api_key.is_empty() {
            vision_auto_done = true;
            macroquad::miniquad::window::request_screen_capture();
            vision_pending = true;
            let say = if lang == Lang::Jp {
                "吾輩は君が何をしているのか見てみたい…"
            } else {
                "吾辈想看看你在做什么…"
            };
            speech_line = Some((say.to_string(), now + 3.0));
            chat_state.history.push(ChatMessage::pet(say));
        }

        // W2 视觉: 轮询新截屏帧 → 视觉模型分析 → 丛雨念出来(按钮触发或 90s 一次)
        #[cfg(target_os = "android")]
        if let Some(jpeg) = macroquad::miniquad::window::take_screen_frame() {
            let should = vision_pending || now - vision_last > 90.0;
            vision_pending = false;
            if should {
                vision_last = now;
                let tx = vision_tx.clone();
                let cfg = vlm_cfg.clone();
                std::thread::spawn(move || {
                    let msg = match crate::chat::analyze_screen(&jpeg, &cfg, lang) {
                        Ok(t) => t,
                        Err(e) => {
                            let prefix = if lang == Lang::Jp {
                                "吾輩は君の画面を見たけど、視覚モデルがまだ設定されていない"
                            } else {
                                "吾辈看到了你的屏幕，但视觉模型还没配好"
                            };
                            format!("{}({})", prefix, e)
                        }
                    };
                    let _ = tx.send(msg);
                });
            }
        }
        if let Ok(msg) = vision_rx.try_recv() {
            chat_state.history.push(ChatMessage::pet(&msg));
            last_reply = msg.clone();
            speech_line = Some((msg.clone(), now + 6.0));
            // 语音: 走远程 TTS, 失败帧循环播兜底
            tts_deadline = now + 10.0;
            let target = tts_result.clone();
            let tts_text = msg;
            std::thread::spawn(move || {
                let result = crate::chat::synthesize_remote(&tts_text).map_err(|e| e.to_string());
                *target.lock().unwrap() = Some(result);
            });
        }

        if verify {
            frame += 1;
            if frame == 120 {
                macroquad::texture::get_screen_data().export_png("pet_screenshot.png");
                println!("verify screenshot saved: pet_screenshot.png");
                break;
            }
        }
        next_frame().await;
    }
}

// ---------------- 鸿蒙宿主壳入�?staticlib) ----------------
// 同一 main.rs 同时作为 [lib] cute_pet_host 编译(crate-type=["staticlib"])�?
// 宿主(ArkTS XComponent + C++ NAPI)加载 .so 后调 pet_entry() 启动渲染线程:
//   pet_entry() �?main()(macroquad 宏生�? �?Window::from_config)
//   �?miniquad-ply::window::start �?native::ohos::run �?spawn 渲染线程(忙等 surface)
//   �?返回; 宿主随后�?XComponent surface �?NAPI �?ohos_surface_created() 喂给渲染线程�?
// W2 视觉模型配置加载(3 级回退):
//   1) Android {filesDir}/vlm_config.json(调试用 adb run-as 注入)
//   2) 构建期内嵌资产 assets/vlm_config.json(发布 APK 开箱即用)
//   3) 环境变量 PET_VLM_BASE_URL / PET_VLM_API_KEY / PET_VLM_MODEL(桌面)
// 字段: base_url / api_key / model(OpenAI 兼容端点)
fn parse_vlm_json(s: &str) -> Option<crate::chat::VlmConfig> {
    use crate::chat::VlmConfig;
    let v = serde_json::from_str::<serde_json::Value>(s).ok()?;
    let get = |k: &str, d: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(|x| x.to_string())
            .unwrap_or_else(|| d.to_string())
    };
    let api_key = v
        .get("api_key")
        .and_then(|x| x.as_str())
        .map(|x| x.to_string())
        .unwrap_or_default();
    if api_key.is_empty() {
        return None;
    }
    Some(VlmConfig {
        base_url: get("base_url", "https://open.bigmodel.cn/api/paas/v4"),
        api_key,
        model: get("model", "glm-4v-flash"),
    })
}

#[cfg(target_os = "android")]
fn load_vlm_config() -> crate::chat::VlmConfig {
    use crate::chat::VlmConfig;
    // 1) 运行时注入
    if let Some(dir) = macroquad::miniquad::window::files_dir() {
        let p = std::path::Path::new(&dir).join("vlm_config.json");
        if let Ok(s) = std::fs::read_to_string(&p) {
            if let Some(cfg) = parse_vlm_json(&s) {
                return cfg;
            }
        }
    }
    // 2) 内嵌资产
    if let Ok(bytes) = load_asset("vlm_config.json") {
        if let Ok(s) = String::from_utf8(bytes) {
            if let Some(cfg) = parse_vlm_json(&s) {
                return cfg;
            }
        }
    }
    // 3) 环境变量
    VlmConfig::from_env().unwrap_or_else(|_| VlmConfig {
        base_url: "https://open.bigmodel.cn/api/paas/v4".into(),
        api_key: String::new(),
        model: "glm-4v-flash".into(),
    })
}

#[cfg(not(target_os = "android"))]
fn load_vlm_config() -> crate::chat::VlmConfig {
    use crate::chat::VlmConfig;
    // 桌面/WASM: 内嵌资产优先(与发布 APK 行为一致), 再环境变量
    if let Ok(bytes) = load_asset("vlm_config.json") {
        if let Ok(s) = String::from_utf8(bytes) {
            if let Some(cfg) = parse_vlm_json(&s) {
                return cfg;
            }
        }
    }
    VlmConfig::from_env().unwrap_or_else(|_| VlmConfig {
        base_url: "https://open.bigmodel.cn/api/paas/v4".into(),
        api_key: String::new(),
        model: "glm-4v-flash".into(),
    })
}

// pet_entry() 已移到 pet/host 的薄壳里: C 符号必须定义在 staticlib 自身,
// 放在 rlib 里链接时可能不被拉进来。这里只留逻辑。

// 键盘高度(px, 0=隐藏): ArkTS keyboardHeightChange �?petKeyboard �?此处�?
// 聊天面板布局据此上移避开软键�?surface 保持全尺�? 避免 resize 渲染 bug)�?
#[cfg(target_env = "ohos")]
static KEYBOARD_H: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// 软键盘高度回调。NAPI 侧的 `ohos_keyboard_height` 符号由 pet/host 薄壳导出,
/// 转发到这里。
#[cfg(target_env = "ohos")]
pub fn ohos_set_keyboard_height(px: i32) {
    KEYBOARD_H.store(px, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(target_env = "ohos")]
fn keyboard_height() -> f32 {
    KEYBOARD_H.load(std::sync::atomic::Ordering::Relaxed).max(0) as f32
}

// ---------------- 素材解耦测试(把 §2.2 合规结论固化成断言) ----------------

#[cfg(test)]
mod asset_decoupling_tests {
    use super::*;

    /// 铁律: 核心资产表绝不能含任何第三方版权角色素材。
    /// 商业构建(`--no-default-features`)只用这张表 —— 这条挂了就等于商业包漏带素材。
    #[test]
    fn core_asset_excludes_character_assets() {
        let leaked: Vec<String> = CoreAsset::iter()
            .map(|f| f.as_ref().to_string())
            .filter(|p| is_character_asset(p))
            .collect();
        assert!(leaked.is_empty(), "核心资产表混入了角色素材: {leaked:?}");
    }

    /// 字体等原创 / 合规资产必须始终编入 —— 商业版也要能正常跑 UI。
    #[test]
    fn core_asset_contains_font() {
        assert!(CoreAsset::get("font_wenkai.ttf").is_some(), "字体应始终编入");
        assert!(CoreAsset::get("font_wenkai-OFL.txt").is_some(), "OFL 文本应随字体分发");
    }

    /// 测试 / 开发构建(默认 bundle-murasame)下角色素材照旧可用, 现有体验不变。
    #[cfg(feature = "bundle-murasame")]
    #[test]
    fn char_asset_bundled_in_dev_build() {
        assert!(CharAsset::get("murasame_manifest.json").is_some());
        let n = CharAsset::iter().count();
        assert!(n > 100, "角色素材应含 117 张立绘分层图, 实际 {n}");
    }

    /// `PET_ASSETS_DIR` 应被优先查找 —— 这是跨平台 / 真机的通用逃生口。
    /// 用唯一文件名, 避免与其它并行测试的素材查找互相干扰。
    #[test]
    fn pet_assets_dir_env_is_honored() {
        let probe = "cp_assets_probe_unique.bin";
        let tmp = std::env::temp_dir().join(format!("cp_assets_probe_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        std::fs::write(tmp.join(probe), b"ok").unwrap();

        std::env::set_var("PET_ASSETS_DIR", &tmp);
        let got = load_asset(probe);
        std::env::remove_var("PET_ASSETS_DIR");

        let _ = std::fs::remove_file(tmp.join(probe));
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(got.is_ok(), "PET_ASSETS_DIR 指定的目录应被优先查找");
    }

    /// 商业构建下角色素材不可从嵌入表取得(只能走用户素材目录)。
    #[cfg(not(feature = "bundle-murasame"))]
    #[test]
    fn char_asset_absent_in_commercial_build() {
        assert!(CoreAsset::get("murasame_manifest.json").is_none());
        assert!(CoreAsset::get("murasame_corpus.jsonl").is_none());
        // 缺失时应给出友好提示, 而不是 panic
        let err = load_asset("murasame_manifest.json").unwrap_err().to_string();
        assert!(err.contains("角色素材缺失"), "错误信息应提示放置素材, 实际: {err}");
    }
}
