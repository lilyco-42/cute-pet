//! 桌宠应用层: 状态(AppState)+ 事件(AppEvent)+ 纯更新逻辑(update)。
//!
//! 架构借鉴(见 `docs/gui-architecture-notes.md`): 取 Elm 的 Model/Update 分层
//! 与 Makepad 的事件收敛, 但保留立即模式 —— 更新是 `update(state, now, event, res)`,
//! 只改变状态并产出 [`AppEffect`](副作用描述), 不触碰平台/渲染/音频资源。
//! 交互逻辑因此与输入物理(宏 quad 轮询)和资源(Sound/Texture)解耦, 可单元测试。

use crate::chat::{voice_meta, Lang, Persona, LANG_TOGGLE, LLM_HINT_TEXT};

/// 桌宠的完整可变状态(纯逻辑, 不含渲染/音频资源)。
#[derive(Debug, Clone)]
pub struct AppState {
    /// 当前语言(影响语音库/persona/台词)。
    pub lang: Lang,
    /// 主音量 0..1。
    pub volume: f32,
    /// 音量指示条显示到此刻(秒)。
    pub vol_show_until: f32,
    /// 语音轮播索引(点击/E 键/兜底依次播放下一条)。
    pub voice_idx: usize,
    /// 当前表情(face id)。
    pub face: String,
    /// 当前服装。
    pub dress: String,
    /// 差分(1/2)。
    pub diff: u32,
    /// 最近一次回复文本(TTS 失败兜底时气泡显示它)。
    pub last_reply: String,
    /// 台词气泡: (文本, 显示到此刻)。
    pub speech_line: Option<(String, f32)>,
    /// 口型动画持续到此刻(发声时触发)。
    pub talk_until: f32,
    /// 眨眼循环计时。
    pub blink_cycle: f32,
    /// 眨眼动画剩余时间(0 = 睁眼)。
    pub blink_phase: f32,
    /// TTS 兜底截止时刻: 提交回复时置 now+10, 超时未出结果则播内嵌语音。
    pub tts_deadline: f32,
    /// F3 调试面板开关。
    pub debug_info: bool,
    /// 待播放的语料语音(ffmpeg 转码后播放)。
    pub pending_voice: Option<String>,
    /// 点击互斥计时(防连击)。
    pub click_since: f32,
    /// 聊天历史(最近 200 条)。
    pub history: Vec<ChatTurn>,
}

/// 聊天历史中的一轮(用户/丛雨)。
#[derive(Debug, Clone)]
pub struct ChatTurn {
    pub from_user: bool,
    pub text: String,
}

impl ChatTurn {
    pub fn user(text: impl Into<String>) -> Self {
        Self { from_user: true, text: text.into() }
    }
    pub fn pet(text: impl Into<String>) -> Self {
        Self { from_user: false, text: text.into() }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            lang: Lang::Zh,
            volume: 1.0,
            vol_show_until: 0.0,
            voice_idx: 0,
            face: "01".to_string(),
            dress: "私服".to_string(),
            diff: 1,
            last_reply: String::new(),
            speech_line: None,
            talk_until: 0.0,
            blink_cycle: 0.0,
            blink_phase: 0.0,
            tts_deadline: 0.0,
            debug_info: false,
            pending_voice: None,
            click_since: 0.0,
            history: Vec::new(),
        }
    }
}

/// 应用事件(由平台层收集输入转换而来)。
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// 数字键选择表情(1..=9)。
    Face(u32),
    /// 切换服装(D)。
    ToggleDress,
    /// 切换差分(Space)。
    ToggleDiff,
    /// 切换语言(L)。
    ToggleLang,
    /// 快速说一句(E)。
    QuickSpeak,
    /// 音量调节: +1 / -1(桌面 =/= 键, Android 触摸屏两侧)。
    Volume(i32),
    /// 截图(F2)。
    Screenshot,
    /// 切换调试面板(F3)。
    ToggleDebug,
    /// 点击桌宠(说话)。
    Click,
    /// 聊天提交(输入框/快捷问题/发送按钮)。
    ChatSubmitted(String),
    /// 语料语音待播放(pending_voice 就绪)。
    PlayPendingVoice,
    /// TTS 结果返回(成功 wav / 失败)。
    TtsResult(Result<Vec<u8>, String>),
    /// TTS 超时触发兜底。
    TtsTimeout,
}

/// 副作用描述: 更新函数不直接触碰资源, 只声明"要做什么", 由平台层执行。
/// 语音类副作用携带**原始索引**, 平台层负责 `idx % 库长` 定位音源。
#[derive(Debug, Clone)]
pub enum AppEffect {
    /// 播放中文问候语音库(点击/兜底)。
    PlayCnVoice(usize),
    /// 播放日文原声反应库(点击)。
    PlayJpVoice(usize),
    /// 播放预置问答第 idx 条语音(可能不存在, 由平台层跳过)。
    PlayPresetVoice(usize),
    /// 远程 TTS 合成文本(异步, 结果经 TtsResult 事件回灌)。
    Synthesize(String),
    /// 桌面 ffmpeg 播放语料语音(Android/鸿蒙无 ffmpeg, 平台层忽略)。
    PlayCorpusVoice(String),
    /// 打开 LLM 渠道页。
    OpenLlmHint,
    /// 端侧朗读文本(桌面: 不动作; WASM: JS 拦截 SPEAK 前缀)。
    SpeakWeb(String),
    /// 日志。
    Log(String),
}

/// 平台层持有的只读资源(避免 AppState 依赖 Sound/Texture)。
#[derive(Clone, Copy)]
pub struct Resources<'a> {
    pub persona_zh: &'a Persona,
    pub persona_jp: &'a Persona,
    pub preset_kws: &'a [String],
    pub preset_answers: &'a [String],
    /// 语音库长度(平台层构造): (中文问候, 日文原声, 预置语音)。
    pub lib_len: (usize, usize, usize),
}

impl AppState {
    /// 追加聊天消息(截断到最近 200 条)。
    fn push(&mut self, turn: ChatTurn) {
        self.last_reply = turn.text.clone();
        if self.history.len() >= 200 {
            self.history.remove(0);
        }
        self.history.push(turn);
    }

    /// 说话台词气泡 + 口型动画(统一入口)。
    fn say(&mut self, now: f32, text: String, bubble_secs: f32) {
        self.talk_until = now + 2.0;
        self.speech_line = Some((text, now + bubble_secs));
    }

    /// 追加一条"丛雨说话"轮, 并记录 last_reply。
    fn pet_reply(&mut self, text: String) {
        self.push(ChatTurn::pet(&text));
    }
}

/// 更新函数: 处理一个事件, 修改状态并产出副作用。
/// `now` 为当前时刻(秒)。
pub fn update(state: &mut AppState, now: f32, event: &AppEvent, res: &Resources) -> Vec<AppEffect> {
    match event {
        AppEvent::Face(i) => {
            state.face = face_id(*i);
            vec![]
        }
        AppEvent::ToggleDress => {
            state.dress = if state.dress == "私服" { "洋装".to_string() } else { "私服".to_string() };
            vec![AppEffect::Log(format!("[dress] {}", state.dress))]
        }
        AppEvent::ToggleDiff => {
            state.diff = if state.diff == 1 { 2 } else { 1 };
            vec![AppEffect::Log(format!("[diff] {}", state.diff))]
        }
        AppEvent::ToggleLang => {
            state.lang = if state.lang == Lang::Zh { Lang::Jp } else { Lang::Zh };
            state.pet_reply(format!("(已切换为{})", state.lang.label()));
            vec![]
        }
        AppEvent::QuickSpeak => speak_once(state, now, true),
        AppEvent::Click => speak_once(state, now, false),
        AppEvent::Volume(d) => {
            state.volume = (state.volume + *d as f32 * 0.1).clamp(0.0, 1.0);
            state.vol_show_until = now + 1.5;
            vec![AppEffect::Log(format!("[vol] {:.0}%", state.volume * 100.0))]
        }
        AppEvent::Screenshot => vec![AppEffect::Log("[shot] screenshot".to_string())],
        AppEvent::ToggleDebug => {
            state.debug_info = !state.debug_info;
            vec![]
        }
        AppEvent::ChatSubmitted(input) => handle_chat(state, now, input, res),
        AppEvent::PlayPendingVoice => match state.pending_voice.take() {
            Some(v) => vec![AppEffect::PlayCorpusVoice(v)],
            None => vec![],
        },
        AppEvent::TtsResult(Ok(wav)) => {
            vec![AppEffect::Log(format!("[tts] 播放克隆音色 {} KB", wav.len() / 1024))]
        }
        AppEvent::TtsResult(Err(e)) => {
            state.push(ChatTurn::pet("(抱歉, 语音合成失败)".to_string()));
            let mut v = fallback_voice(state, now);
            v.insert(0, AppEffect::Log(format!("[tts] 合成失败: {e}")));
            v
        }
        AppEvent::TtsTimeout => {
            state.tts_deadline = 0.0;
            fallback_voice(state, now)
        }
    }
}

/// 兜底语音(LLM/TTS 失败或超时): 轮播中文问候, 显示最近回复气泡。
fn fallback_voice(state: &mut AppState, now: f32) -> Vec<AppEffect> {
    let idx = state.voice_idx;
    state.voice_idx = idx.wrapping_add(1);
    if let Some((_, face, _zh, _jp)) = voice_meta("greeting_01") {
        state.face = (*face).to_string();
        let bubble = if state.last_reply.is_empty() {
            "吾辈在这里哦".to_string()
        } else {
            state.last_reply.clone()
        };
        state.say(now, bubble, 3.0);
    }
    vec![AppEffect::PlayCnVoice(idx)]
}

/// 说一句(点击桌宠 / E 键): 轮播当前语言语音库 + 更新表情与台词气泡。
/// `is_key` 为 true 时不受点击互斥限制(E 键可连按)。
fn speak_once(state: &mut AppState, now: f32, is_key: bool) -> Vec<AppEffect> {
    if !is_key && now - state.click_since < 0.4 {
        return Vec::new();
    }
    state.click_since = now;

    let idx = state.voice_idx;
    state.voice_idx = idx.wrapping_add(1);

    let effect = match state.lang {
        Lang::Zh => AppEffect::PlayCnVoice(idx),
        Lang::Jp => AppEffect::PlayJpVoice(idx),
    };
    // 用元数据更新表情 + 气泡(平台层负责按 idx%库长 取音源, 这里只取台词模板)。
    let name = if state.lang == Lang::Zh { "greeting_01" } else { "mur001_013" };
    if let Some((_, face, zh, jp)) = voice_meta(name) {
        state.face = (*face).to_string();
        let text = if state.lang == Lang::Jp { jp.unwrap_or(zh) } else { zh };
        state.say(now, text.to_string(), 4.0);
    }
    vec![effect]
}

/// 聊天提交处理: 快捷按钮 → 预置问答 → persona 语料(LLM → 语料兜底)。
fn handle_chat(state: &mut AppState, now: f32, input: &str, res: &Resources) -> Vec<AppEffect> {
    let mut effects = Vec::new();
    let input = input.trim();
    if input.is_empty() {
        return effects;
    }

    // 语言切换快捷按钮
    if input == LANG_TOGGLE {
        state.lang = if state.lang == Lang::Zh { Lang::Jp } else { Lang::Zh };
        state.pet_reply(format!("(已切换为{}模式)", state.lang.label()));
        return effects;
    }
    // LLM 免费模型提示
    if input == LLM_HINT_TEXT {
        effects.push(AppEffect::OpenLlmHint);
        return effects;
    }

    state.push(ChatTurn::user(input.to_string()));

    // 预置问答: 否定排除 + 更长关键词优先。
    if let Some(idx) = crate::chat::preset_match(res.preset_kws, input) {
        let ans = res.preset_answers[idx].clone();
        state.pet_reply(ans.clone());
        effects.push(AppEffect::SpeakWeb(ans.clone()));
        if res.lib_len.2 > 0 {
            effects.push(AppEffect::PlayPresetVoice(idx));
            state.say(now, ans, 3.5);
        } else {
            effects.push(AppEffect::Synthesize(ans));
            state.tts_deadline = now + 10.0;
        }
        return effects;
    }

    // persona 回复: 多轮上下文(最近 10 轮) → LLM → 语料兜底。
    let persona = if state.lang == Lang::Zh { res.persona_zh } else { res.persona_jp };
    let history: Vec<(String, String)> = state
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
    let (text, voice) = match persona.respond_llm(&history, input) {
        Ok(s) => (s, None),
        Err(_) => match persona.respond_corpus(input) {
            Some((t, v)) => {
                let t = if state.lang == Lang::Zh {
                    match crate::chat::translate_to_chinese(&t) {
                        Some(zh) => zh,
                        None => t,
                    }
                } else {
                    t
                };
                (t, Some(v))
            }
            None => ("(无回应)".to_string(), None),
        },
    };
    state.pet_reply(text.clone());

    // 语音: 语料原声(桌面 ffmpeg)优先, 否则远程 TTS。
    match voice {
        Some(v) if state.pending_voice.is_none() => {
            state.pending_voice = Some(v);
        }
        _ => {
            effects.push(AppEffect::Synthesize(text));
            state.tts_deadline = now + 10.0;
        }
    }
    effects
}

/// 数字键 1..=9 → 表情 id。
pub fn face_id(idx: u32) -> String {
    const FACES: [&str; 9] = ["01", "03", "04", "13", "14", "19", "21", "02", "20"];
    FACES[idx as usize % FACES.len()].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res() -> Resources<'static> {
        static ZH: std::sync::OnceLock<Persona> = std::sync::OnceLock::new();
        static JP: std::sync::OnceLock<Persona> = std::sync::OnceLock::new();
        Resources {
            persona_zh: ZH.get_or_init(|| Persona::murasame_from_corpus_content("").unwrap()),
            persona_jp: JP.get_or_init(|| Persona::murasame_from_corpus_content("").unwrap()),
            preset_kws: &[],
            preset_answers: &[],
            lib_len: (10, 5, 0),
        }
    }

    #[test]
    fn volume_clamps() {
        let mut s = AppState::default();
        for _ in 0..20 {
            update(&mut s, 0.0, &AppEvent::Volume(1), &res());
        }
        assert_eq!(s.volume, 1.0);
        for _ in 0..20 {
            update(&mut s, 0.0, &AppEvent::Volume(-1), &res());
        }
        assert_eq!(s.volume, 0.0);
        assert!(s.vol_show_until > 0.0);
    }

    #[test]
    fn toggle_lang_switches_and_logs() {
        let mut s = AppState::default();
        assert_eq!(s.lang, Lang::Zh);
        update(&mut s, 0.0, &AppEvent::ToggleLang, &res());
        assert_eq!(s.lang, Lang::Jp);
        assert_eq!(s.history.len(), 1);
    }

    #[test]
    fn quick_speak_advances_voice_idx() {
        let mut s = AppState::default();
        let e = update(&mut s, 0.0, &AppEvent::QuickSpeak, &res());
        assert!(s.speech_line.is_some());
        assert!(matches!(e.first(), Some(AppEffect::PlayCnVoice(_))));
        let idx0 = s.voice_idx;
        update(&mut s, 0.0, &AppEvent::QuickSpeak, &res());
        assert!(s.voice_idx > idx0);
    }

    #[test]
    fn click_is_debounced() {
        let mut s = AppState::default();
        update(&mut s, 1.0, &AppEvent::Click, &res());
        let e = update(&mut s, 1.1, &AppEvent::Click, &res());
        assert!(e.is_empty()); // 0.4s 内重复点击被忽略
        let e = update(&mut s, 1.6, &AppEvent::Click, &res());
        assert!(!e.is_empty());
    }

    #[test]
    fn face_id_cycles() {
        assert_eq!(face_id(0), "01");
        assert_eq!(face_id(1), "03");
        assert_eq!(face_id(9), "01");
    }

    #[test]
    fn chat_submit_sets_tts_deadline() {
        let mut s = AppState::default();
        let e = update(&mut s, 100.0, &AppEvent::ChatSubmitted("你好".to_string()), &res());
        assert!(s.tts_deadline > 100.0);
        assert!(e.iter().any(|e| matches!(e, AppEffect::Synthesize(_))));
        assert_eq!(s.history.len(), 2); // user + pet
    }
}
