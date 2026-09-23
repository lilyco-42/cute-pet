//! UI 国际化: 界面文案常量(中/日)与切换。

use lazy_ply::components::{chat_panel, ChatPanelState};
use crate::chat::{Lang, LANG_TOGGLE, LLM_HINT_TEXT};

/// 免费模型渠道信息页(放 GitHub 或项目文档, 便于持续维护)。
pub(crate) const LLM_HINT_URL: &str = "https://github.com/lazy-plxy/cute-pet/blob/main/docs/free-llm.md";

/// W2 视觉: 聊天面板快捷问题文案(main.rs 拦截处理: 截屏让丛雨"看着你")。
pub(crate) const VISION_QUESTION: &str = "🔍 看看我在干嘛";
pub(crate) const VISION_QUESTION_JP: &str = "🔍 何を見てる？";
/// 语言切换按钮文案(中/日)
pub(crate) const LANG_TOGGLE_JP: &str = "🌐 言語切替";
/// LLM 免费模型提示(日)
pub(crate) const LLM_HINT_TEXT_JP: &str = "AI 会話: 無料モデル → NVIDIA NIM · OpenRouter · 商湯 (タップで表示)";
/// 快捷问题列表(中/日, 与 lazy-ply chat_panel 默认一致)
pub(crate) const QUICK_ZH: &[&str] = &[
    "在吗？",
    "吃饭了吗？",
    "想我了吗？",
    "心情不好",
    "晚安",
    "你会一直陪我吗？",
    "🌐 切换语言",
    "🔍 看看我在干嘛",
];
pub(crate) const QUICK_JP: &[&str] = &[
    "いる？",
    "ご飯食べた？",
    "会いたかった？",
    "機嫌悪い",
    "おやすみ",
    "ずっと一緒にいてくれる？",
    "🌐 言語切替",
    "🔍 何を見てる？",
];
pub(crate) const PLACEHOLDER_JP: &str = "日本語で入力…";
pub(crate) const SEND_JP: &str = "送信";

/// 按当前语言切换聊天面板 UI 文案(快捷问题/输入框/发送/LLM 提示)。
pub(crate) fn apply_ui_lang(state: &mut ChatPanelState, lang: Lang) {
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

// ---------------- 测试 ----------------
//
// 这一域没有 GPU / 窗口依赖, 全是常量表与赋值, 所以能在纯 `cargo test` 里跑。
// 盯的是三类"静默失效": 表长度漂移、按字符串拦截的文案漂移、默认值与表不一致。

#[cfg(test)]
mod tests {
    use super::*;

    /// 中/日快捷问题必须**逐条对齐**: chat_panel 是按索引渲染按钮的, 数量不一致
    /// 会让切语言时按钮数跳变; 空串会渲染出点不动也看不见文字的按钮。
    #[test]
    fn quick_questions_zh_and_jp_are_aligned() {
        assert_eq!(QUICK_ZH.len(), QUICK_JP.len(), "中/日快捷问题数量必须一致");
        for (i, (zh, jp)) in QUICK_ZH.iter().zip(QUICK_JP.iter()).enumerate() {
            assert!(!zh.trim().is_empty(), "QUICK_ZH[{i}] 是空串");
            assert!(!jp.trim().is_empty(), "QUICK_JP[{i}] 是空串");
        }
    }

    /// 切换语言 / 视觉功能是**按字符串比对**拦截的(`run.rs`: `input == LANG_TOGGLE`、
    /// `input == VISION_QUESTION`)。快捷问题表里的文案一旦和常量漂移, 这两个功能会
    /// 静默失效 —— 点了没反应, 不报错、不打日志, 是最难查的一类 bug。
    #[test]
    fn interceptable_questions_exist_in_quick_lists() {
        assert!(QUICK_ZH.contains(&LANG_TOGGLE), "LANG_TOGGLE 不在 QUICK_ZH 里 → 语言切换按钮失效");
        assert!(QUICK_JP.contains(&LANG_TOGGLE_JP), "LANG_TOGGLE_JP 不在 QUICK_JP 里");
        assert!(QUICK_ZH.contains(&VISION_QUESTION), "VISION_QUESTION 不在 QUICK_ZH 里 → 视觉功能失效");
        assert!(QUICK_JP.contains(&VISION_QUESTION_JP), "VISION_QUESTION_JP 不在 QUICK_JP 里");
    }

    /// 面板默认文案(中文)与 QUICK_ZH 必须同源 —— 否则首屏一套按钮、切回中文后
    /// 另一套(lazy-ply 的 Default 是独立维护的副本, 最容易漂移)。
    #[test]
    fn default_panel_matches_zh_table() {
        let d = ChatPanelState::default();
        assert_eq!(d.quick_questions, QUICK_ZH);
        assert_eq!(d.send_label, "发送");
        assert_eq!(d.input_placeholder, "说点什么…");
    }

    /// 切语言必须把四个字段全换到目标语言, 且能切回来(往返一致)。
    #[test]
    fn apply_ui_lang_switches_every_panel_string() {
        let mut s = ChatPanelState::default();
        s.llm_hint = Some(LLM_HINT_TEXT);

        apply_ui_lang(&mut s, Lang::Jp);
        assert_eq!(s.quick_questions, QUICK_JP);
        assert_eq!(s.input_placeholder, PLACEHOLDER_JP);
        assert_eq!(s.send_label, SEND_JP);
        assert_eq!(s.llm_hint, Some(LLM_HINT_TEXT_JP));

        apply_ui_lang(&mut s, Lang::Zh);
        assert_eq!(s.quick_questions, QUICK_ZH);
        assert_eq!(s.input_placeholder, "说点什么…");
        assert_eq!(s.send_label, "发送");
        assert_eq!(s.llm_hint, Some(LLM_HINT_TEXT));
    }

    /// 没开 LLM 提示(`llm_hint == None`)时切语言不该把它变成有值 ——
    /// 那会让提示行凭空出现在没配提示的界面上。
    #[test]
    fn apply_ui_lang_keeps_hidden_hint_hidden() {
        let mut s = ChatPanelState::default();
        s.llm_hint = None;
        apply_ui_lang(&mut s, Lang::Jp);
        assert!(s.llm_hint.is_none());
        apply_ui_lang(&mut s, Lang::Zh);
        assert!(s.llm_hint.is_none());
    }
}
