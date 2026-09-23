//! 平台差异层: 每个目标平台一个子模块, 编译期由 cfg 选择。
//! 跨平台工具函数(如打开浏览器页)也归这里。

pub(crate) mod linux;
pub(crate) mod macos;
pub(crate) mod windows;

pub(crate) use crate::ui::i18n::LLM_HINT_URL;

#[cfg(not(any(target_os = "android", target_env = "ohos")))]
pub(crate) fn open_llm_hint_page() {
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
pub(crate) fn open_llm_hint_page() {
    // TODO: Android/鸿蒙 用系统意图打开浏览器; 暂不实现(桌面/WASM 为主)
}

