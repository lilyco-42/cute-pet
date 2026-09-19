//! 鸿蒙宿主壳静态库(staticlib)。
//!
//! 导出两个 C 符号给 ArkTS XComponent + C++ NAPI 壳(见 ../host_app/):
//! - `pet_entry()`: 启动 macroquad 主循环 → miniquad-ply ohos backend;
//! - `ohos_keyboard_height(px)`: 软键盘高度回调。
//!
//! 这里刻意保持"薄壳": 渲染逻辑全部在 cute-pet lib 里, 不再用
//! `#[path = "../../src/main.rs"] mod app;` 把二进制的 main.rs include 进来。
//! 带来的三个好处:
//! 1. 依赖树只有一棵 —— 不会再出现桌面版打了 macroquad-ply patch、
//!    鸿蒙版漏打这种漂移;
//! 2. 资产由 lib 的 rust-embed 从 `pet/assets/` 直接嵌入, 不再需要
//!    `pet/host/assets` 这个本机 junction(干净 clone 里没有它, 会编译失败);
//! 3. 改 main.rs 不会再静默搞坏鸿蒙构建。
//!
//! C 符号必须定义在本 crate(而不是依赖的 rlib)里: rlib 中未被 Rust 代码
//! 引用的 `#[no_mangle]` 函数, 链接 staticlib 时可能不会被拉进来, NAPI 侧
//! dlsym 就会落空。
#![cfg(target_env = "ohos")]

/// 启动渲染循环(阻塞)。
#[no_mangle]
pub extern "C" fn pet_entry() {
    cute_pet::start();
}

/// 软键盘高度回调(ArkTS keyboardHeightChange → petKeyboard)。
#[no_mangle]
pub extern "C" fn ohos_keyboard_height(px: i32) {
    cute_pet::ohos_set_keyboard_height(px);
}
