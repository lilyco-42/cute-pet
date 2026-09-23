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
