//! 鸿蒙宿主壳静态库(staticlib)。
//!
//! 通过 `#[path]` 把桌宠的 `src/main.rs` 原样包含为 `mod app`,
//! 其内的 `#[no_mangle] pub extern "C" fn pet_entry()`(ohos 专属)
//! 即本库对外入口符号: 启动 macroquad 主循环 → miniquad-ply ohos backend
//! → 渲染线程(忙等 XComponent surface)。
//!
//! 资产: rust-embed `#[folder="assets/"]` 相对本 crate manifest dir,
//! 由 `pet/host/assets` junction → `pet/assets` 提供。
#![cfg(target_env = "ohos")]

#[path = "../../src/main.rs"]
mod app;
