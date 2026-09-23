//! 丛雨(ムラサメ) 桌宠渲染层 — ply-engine / macroquad:
//! manifest 驱动图层选择 + 逐层 draw_texture_ex 合成 + 待机动画 + 表情切换 + 透明置顶窗口。
//!
//! 域模块化(2026-09-23 重构): 资产注册与 manifest 模型在 [`crate::assets`],
//! 语音映射与播放在 [`crate::voice`], 文案/动画/部件在 [`crate::ui`],
//! 平台差异在 [`crate::platform`]。本文件只保留**装配层**: 窗口配置 +
//! start()/run() 主装配与主循环 + 鸿蒙宿主接口 —— 域逻辑不再堆在这里。
//!
//! 对外入口(见 lib.rs): `start()` / `run()` / `ohos_set_keyboard_height()`。
//! 桌面二进制(src/main.rs)和鸿蒙 staticlib(pet/host)共用 start(),
//! 不再各写一份装配 —— 此前鸿蒙壳只能 #[path] include 整个 main.rs,
//! 要复制一整份依赖树(并依赖本机 junction, 干净 clone 必然构建失败)。
use ply_engine::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use lazy_ply::components::{chat_panel, ChatMessage, ChatPanelEvents, ChatPanelState};
use serde::Deserialize;

use crate::app::{self, AppEvent, AppState};
use crate::assets::*;
use crate::chat::{voice_meta, Lang, Persona, LANG_TOGGLE, LLM_HINT_TEXT};
use crate::ui::anim::*;
use crate::ui::i18n::*;
use crate::ui::widgets::*;
use crate::voice::*;

// 平台模块(原 crate 根级, 现归 platform/), 本文件是子模块, 需显式引入。
#[cfg(target_os = "windows")]
use crate::platform::windows;
#[cfg(target_os = "macos")]
use crate::platform::macos;
#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
use crate::platform::linux;
use crate::platform::open_llm_hint_page;

// ---------------- 窗口 ----------------

pub fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: miniquad::conf::Conf {
            window_title: "cute-pet 桌宠".to_owned(),
            window_width: 600,
            window_height: 850,
            high_dpi: true,
            window_resizable: false,
            // 关闭 MSAA: 模拟器宿主 GPU 透传(如 AMD Translator)不提供
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

/// 角色素材缺失时, 在窗口内持续绘制中文"请放置素材"引导面板。
///
/// 商业包(无 `bundle-murasame`)默认不含柚子社版权立绘, 缺素材是预期而非崩溃:
/// 原先直接 `return` 导致黑屏, 现改为画出引导(始终随包分发的 `font_wenkai.ttf` 渲染中文),
/// 告诉用户把素材放到哪个目录, 覆盖桌面 / 原生 Android APK / WebView 壳 wasm 全平台。
async fn draw_missing_asset_guide_loop() {
    let msg = character_asset_missing_msg(MANIFEST_PATH);

    // 字体为核心素材, 商业构建也随包, 必然可用; 万一缺失则退回纯日志(仍不 panic)。
    let font = match load_asset("font_wenkai.ttf")
        .ok()
        .and_then(|b| macroquad::text::load_ttf_font_from_bytes(&b).ok())
    {
        Some(f) => f,
        None => {
            eprintln!("[cute-pet] 引导字体缺失, 无法在窗口内绘制, 仅打印到日志:\n{msg}");
            loop {
                next_frame().await;
            }
        }
    };

    let ui_scale = if cfg!(any(target_os = "android", target_env = "ohos")) {
        (screen_width() / 400.0).clamp(1.0, 3.5)
    } else {
        1.0
    };
    let font_size = (18.0 * ui_scale).round() as u16;
    let pad = 22.0 * ui_scale;
    let line_gap = font_size as f32 * 0.5;
    let wrap_w = screen_width() - pad * 2.0;

    loop {
        // 深色半透明底, 保证中文可读(原生 Android 透明窗口下也清晰可见)
        clear_background(MacroquadColor::new(0.04, 0.05, 0.09, 0.94));
        draw_rectangle(
            pad * 0.5,
            pad * 0.5,
            screen_width() - pad,
            screen_height() - pad,
            MacroquadColor::new(0.10, 0.12, 0.18, 0.96),
        );

        let mut top = pad + font_size as f32;
        for (i, line) in msg.lines().enumerate() {
            let wrapped = macroquad::text::wrap_text(line, Some(&font), font_size, 1.0, wrap_w);
            let dims = macroquad::text::measure_multiline_text(
                &wrapped,
                Some(&font),
                font_size,
                1.0,
                Some(1.4),
            );
            // 首行(标题)高亮, 其余常规白字
            let color = if i == 0 {
                MacroquadColor::new(1.0, 0.82, 0.36, 1.0)
            } else {
                MacroquadColor::new(0.95, 0.97, 1.0, 1.0)
            };
            macroquad::text::draw_multiline_text_ex(
                &wrapped,
                pad,
                top + dims.offset_y,
                Some(1.4),
                macroquad::text::TextParams {
                    font: Some(&font),
                    font_size,
                    color,
                    ..Default::default()
                },
            );
            top += dims.height + line_gap;
        }
        next_frame().await;
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
    // 优先加载第三方版权角色(丛雨); 缺失时回退到内置原创默认吉祥物(随所有构建分发, 合规),
    // 两者皆缺(极端情况)才在窗口内绘制"请放置素材"引导。商业包默认走默认吉祥物路径, 不再黑屏。
    let (manifest_bytes, layer_dir): (Vec<u8>, &str) =
        match load_character_asset_or_warn(MANIFEST_PATH) {
            Some(b) => (b, LAYER_DIR),
            None => match load_asset(DEFAULT_PET_MANIFEST).ok() {
                Some(b) => {
                    eprintln!(
                        "[cute-pet] 未找到第三方版权角色素材, 回退到内置默认吉祥物(原创, 可自由使用)。"
                    );
                    (b, DEFAULT_PET_LAYER_DIR)
                }
                None => {
                    eprintln!("[cute-pet] 未找到任何角色素材, 在窗口内绘制放置引导。");
                    draw_missing_asset_guide_loop().await;
                    return; // draw_missing_asset_guide_loop 内部为不返回的渲染循环, 此行仅作控制流兜底
                }
            },
        };
    let mut manifest: Manifest = serde_json::from_slice(&manifest_bytes).expect("解析 manifest.json");
    println!("加载角色: {} ({}) voice={}", manifest.name_cn, manifest.character, manifest.voice_code);

    let set_meta = manifest.sets.remove("a").expect("缺少 set a");

    // 角色轮廓 bbox: 让窗口贴合立绘(顶部透明区域裁掉)
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
    // 原始立绘尺寸(不含内边距), 供渲染缩放使用
    let sprite_w = bbox_w;
    let sprite_h = bbox_h;
    // 内边距(画布坐标): 窗口=舞台(顶部气泡区 + 底部聊天控件区 + 中部立绘),
    // 聊天 UI 不再覆盖立绘
    const PAD: u32 = 90;
    const PAD_TOP: u32 = 480;    // 顶部气泡区
    const PAD_BOTTOM: u32 = 540; // 底部聊天控件区
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
        if let Ok(bytes) = load_asset(&format!("{layer_dir}/a_{id}.png")) {
            textures.insert(item.layer_id, Texture2D::from_file_with_format(&bytes, None));
        }
    }
    println!("已加载{} 层纹理", textures.len());

    // 语音库: 按 VOICE_META 表加载, 中文克隆问候(greeting) + 日文原声反应(mur001) 分开
    let mut cn_voices: Vec<(String, Sound)> = Vec::new(); // 中文(点击/兜底用)
    let mut jp_voices: Vec<(String, Sound)> = Vec::new(); // 日文原声反应
    for (name, _face, _zh, _jp) in crate::chat::VOICE_META {
        let path = if name.starts_with("greeting_") {
            // greeting 为 ogg: WASM 走浏览器 decodeAudioData, 原生支持 vorbis
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
    // 预置对话库: 中文问答(文本独立于语音, 保证回复一定中文)
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
            // 桌面: PET_VOICE_DIR 指向外部语音目录时用其 dialog.txt; 否则(含 Android)用内嵌预置问答
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
    // 语音: 与问答一一对应(dialog_preset 75 条 = voice_preset/fei00-74)。
    // 用 Vec<Option<Sound>> 按 idx 对齐: 某条加载失败时占位 None(播放时跳过),
    // 保证语音与文本永远不错位。
    let mut preset_sounds: Vec<Option<Sound>> = Vec::new();
    // 内嵌 voice_preset 按 idx 对齐加载
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

    // 聊天层: 丛雨 persona(LLM env 门控 + 语料兜底) + CJK 字体
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
    // 台词气泡用字体(独立 Font 实例, 与 ply 同字, macroquad draw_text_ex 直绘)
    let mq_font = macroquad::text::load_ttf_font_from_bytes(&font_bytes).ok();
    let font_data: &'static [u8] = Box::leak(font_bytes.into_boxed_slice());
    let font_asset: &'static FontAsset = Box::leak(Box::new(FontAsset::Bytes {
        file_name: "font_wenkai.ttf",
        data: font_data,
    }));
    let mut ply = Ply::<()>::new(font_asset).await;
    // 聊天面板(lazy-ply 组件): 气泡历史 + 快捷问题 + 输入框
    let mut chat_state = ChatPanelState::default();
    // 未配置 LLM Key → 面板底部提示免费模型渠道(NVIDIA NIM / OpenRouter / 商汤)
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
    // 远程 TTS 合成(丛雨克隆音色): 后台线程拉 wav, 帧循环播放。
    // 结果携带成败: Ok(wav) 播放克隆音色; Err 立即播放内嵌兜底语音(保证点击/回复必有声音)
    let tts_result: Arc<Mutex<Option<Result<Vec<u8>, String>>>> = Arc::new(Mutex::new(None));
    // 台词气泡: (文本, 显示到此刻), 点击桌宠说话时显示在角色旁
    let mut speech_line: Option<(String, f32)> = None;
    // 互斥播放: 记录正在/刚播放的声音, 新播放前停掉它(防重叠)
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
    // F3 调试面板: 显示语音库加载数(诊断用)
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
    linux::make_transparent_pet_window("cute-pet 桌宠");

    // 拖拽移动
    let mut dragging = false;
    let mut grab_mx = 0.0f32;
    let mut grab_my = 0.0f32;
    // 点击互动: 说完随机台词 + 切表情
    let mut click_since = 0.0f32;

    // 眨眼/口型动画: 眨眼计时 + 说话口型计时
    let mut blink_cycle = 0.0f32;
    let mut blink_phase = 0.0f32;   // 眨眼动画剩余时间(0 = 睁眼)
    let mut talk_until = 0.0f32;    // 口型动画持续到此刻(发声时触发)

    // 主音量(0.0~1.0): 桌宠页面内可调(桌面 =/= 键, Android 触摸屏两侧)
    let mut master_volume: f32 = 1.0;
    let mut vol_show_until = 0.0f32;   // 音量指示条显示到此刻

    loop {
        let now = macroquad::time::get_time() as f32;
        // 透明背景: alpha=0, 由 DWM 合成到桌面
        clear_background(MacroquadColor::new(0.0, 0.0, 0.0, 0.0));
        // Android/鸿蒙 和风背景(不透明窗口, lazy-ply 组件); 桌面透明窗口不画
        #[cfg(any(target_os = "android", target_env = "ohos"))]
        lazy_ply::components::pet_background(now, screen_width(), screen_height());

        // 主音量调节(桌宠页面内): 桌面 `-`/`=` 或 `[`/`]`, Android 触摸屏两侧
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

        // Android 触摸经 macroquad 映射为鼠标事件(单指=鼠标), 用鼠标释放统一处理:
        // 上半屏左右边缘 = 音量; 角色范围内 = 说话。不用 touches() 的 Started 判断
        // (adb/真机 DOWN+UP 可能同帧到达, Started 阶段会丢失)。
        if is_mouse_button_released(MouseButton::Left) && now - click_since > 0.4 {
            click_since = now;
            let (mx, my) = mouse_position();
            #[cfg(any(target_os = "android", target_env = "ohos"))]
            {
        // 上半屏左右边缘 = 音量; 角色范围内 = 说话。不用 touches() 的 Started 判断
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
                    // 点击立绘中部(避开顶部气泡区与底部聊天控件区, 且横向在角色范围内)
                    let ui_scale_click = (screen_width() / 400.0).clamp(1.0, 3.5);
                    let in_ui_band = my < 150.0 * ui_scale_click + 10.0 || my > screen_height() - 190.0 * ui_scale_click - 10.0;
                    let in_char_x = mx >= char_rect.0 - 30.0 && mx <= char_rect.0 + char_rect.2 + 30.0;
                    if !in_ui_band && in_char_x {
                        // 中文模式: 克隆问候(greeting); 日文模式: 原声反应(mur001)
                        let active: &[(String, Sound)] = if lang == Lang::Zh { &cn_voices } else { &jp_voices };
                        if !active.is_empty() {
                            let (name, sound) = &active[voice_idx % active.len()];
                            play_voice_vol(sound, master_volume, &mut last_sound);
                            voice_idx += 1;
                            talk_until = now + 2.0; // 发声时触发口型动画
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
                // 点击立绘中部(避开顶部气泡区与底部聊天控件区) → 轮播台词 + 切表情
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
        // E: 快速说一句(轮播, 按当前语言); Enter: 聊天输入
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
        // (聊天输入已交给 lazy-ply chat_panel: 事件在 UI 帧内收集, 回复处理在立绘绘制后)
        if is_key_pressed(KeyCode::Escape) {
            break;
        }

        // 播放回复语音(ffmpeg m4a→wav → 加载 → 播放; 仅桌面 — Android/鸿蒙无 ffmpeg, 走 TTS/兜底)
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
                            talk_until = now + 2.0; // 发声时触发口型动画
                        }
                    }
                }
            }
            #[cfg(any(target_os = "android", target_env = "ohos"))]
            let _ = v;
        }
        // 播放远程 TTS 合成的丛雨克隆音色; 失败立即播放内嵌兜底语音(点击/回复必有声音)
        if let Some(result) = tts_result.lock().unwrap().take() {
            match result {
                Ok(wav) => {
                    if let Ok(s) = load_sound_from_bytes(&wav).await {
                        play_voice_vol(&s, master_volume, &mut last_sound);
                        talk_until = now + 2.0; // 发声时触发口型动画
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
        // TTS 超时(网络卡死)兜底: 提交回复后 tts_deadline 内无结果 → 播内嵌语音
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

        // 眨眼/口型动画: 计算本次绘制用表情
        //   - 说话中(now < talk_until): 缓慢交替 基础脸 ↔ 说话口型脸(≈2.5Hz,
        //     说话 twin 已统一为睁眼版(39), 避免说话时眼睛高频变化像疯狂眨眼)
        //   - 空闲: 每 2.6~4.2s 眨眼一次(人类眨眼频率 ≈3~5s, 闭眼脸 120ms)
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

        // 角色定位(每帧按当前屏幕尺寸计算 — 桌面=窗口, Android/WASM=屏幕, 自适应)
        const UI_TOP_PX: f32 = 150.0;   // 气泡区高度
        const UI_BOTTOM_PX: f32 = 190.0; // 控件区高度(按钮+输入框)
        {
            let scr_w = screen_width();
            let scr_h = screen_height();
            let avail_h = (scr_h - UI_TOP_PX - UI_BOTTOM_PX).max(200.0);
            let render_scale = (avail_h / (sprite_h as f32 * SCALE)).min(1.0) * 0.90;
            let char_w = sprite_w as f32 * SCALE * render_scale;
            let char_h = sprite_h as f32 * SCALE * render_scale;
            let char_x = (scr_w - char_w) / 2.0;
            // 经验校准: 立绘实际渲染比 bbox 计算低约 6% 高度, 按比例上移
            // 0.62: 角色中心略偏下, 靠近底部控件区(构图平衡)
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

        // 台词气泡: 点击桌宠说话的台词, 显示在角色头顶上方
        if let Some((text, until)) = speech_line.take() {
            if now < until {
                if let Some(font) = &mq_font {
                    draw_speech_bubble(&text, font, char_rect.0 + char_rect.2 / 2.0, char_rect.1);
                }
                speech_line = Some((text, until));
            }
        }

        // 聊天面板(lazy-ply 组件): 气泡历史 + 快捷问题 + 输入框, 覆盖在立绘上方
        #[cfg(any(target_os = "android", target_env = "ohos"))]
        {
            // Android/HarmonyOS: miniquad dpi_scale 可能返回 1.0(按物理像素渲染),
            // 用设计宽度 400dp 推算 UI 缩放, 统一放大按钮/输入框/气泡文字到可触控尺寸
            use lazy_ply::components::config::{Attrs, ButtonConfig, ButtonStateConfig, ChatPanelConfig, Style, TextFieldConfig};
            let ui_scale = (screen_width() / 400.0).clamp(1.0, 3.5);
            let mut ui = ply.begin();
            // 键盘避让: 压缩布局高度让聊天面板上移(鸿蒙键盘弹出时)
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
            // 桌面/WASM: 浅色和风背景上的高对比样式(深紫按钮/白底输入框/不透明气泡)
            use lazy_ply::components::config::{Attrs, ButtonConfig, ButtonStateConfig, ChatPanelConfig, Style, TextFieldConfig};
            let mut ui = ply.begin();
            let _g = Style::with(
                Attrs {
                    chat_panel: Some(ChatPanelConfig {
                        // 注意: 不设 background — 面板是全屏容器, 不透明背景会盖住立绘
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
        // 音量指示条画在聊天面板之后(Android 上不被控件盖住)
        if now < vol_show_until {
            draw_volume_indicator(master_volume);
        }
        // F3 调试面板: 语音库加载数(诊断"没声音/不读"用)
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
        // 统一处理输入: 聊天面板事件(快捷按钮/输入框) → 回复(气泡在下一帧显示)
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
            // LLM 免费模型提示: 点击在浏览器打开信息页 (中/日文案)
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
            // 预置问答: 统一匹配逻辑(否定排除 + 更长关键词优先 + 忽略标点, 见 chat::preset_match)
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
                    // 无预置语音 → 远程 TTS(失败/超时帧循环播兜底)
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
            // persona 回复: 按当前语言选 persona(中文语料已过滤假名)
            let persona = if lang == Lang::Zh { &persona_zh } else { &persona_jp };
            // 多轮上下文: 取最近 10 轮(用户/丛雨, 旧→新), 让 LLM 前言搭后语
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
            // 语料语音: 桌面 ffmpeg 播放; Android/鸿蒙无 ffmpeg 丢弃走 TTS
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
                // 远程 TTS 合成丛雨克隆音色(后台线程, 结果带成败 → 失败帧循环播兜底)
                tts_deadline = now + 10.0;
                let target = tts_result.clone();
                let tts_text = text.clone();
                std::thread::spawn(move || {
                    let result = crate::chat::synthesize_remote(&tts_text).map_err(|e| e.to_string());
                    *target.lock().unwrap() = Some(result);
                });
            }
        }

        // 验证模式: 渲染 2 秒后截图并退出
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

// ---------------- 鸿蒙宿主壳入口(staticlib) ----------------
// 同一 main.rs 同时作为 [lib] cute_pet_host 编译(crate-type=["staticlib"])。
// 宿主(ArkTS XComponent + C++ NAPI)加载 .so 后调 pet_entry() 启动渲染线程:
//   pet_entry() → main()(macroquad 宏生成, 即 Window::from_config)
//   → miniquad-ply::window::start → native::ohos::run → spawn 渲染线程(忙等 surface)
//   → 返回; 宿主随后把 XComponent surface 经 NAPI 调 ohos_surface_created() 喂给渲染线程。
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

// 键盘高度(px, 0=隐藏): ArkTS keyboardHeightChange → petKeyboard → 此处。
// 聊天面板布局据此上移避开软键盘(surface 保持全尺寸, 避免 resize 渲染 bug)。
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
