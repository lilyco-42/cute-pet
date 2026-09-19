//! cute-pet 库: 聊天/语气层(可单独测试) + 应用状态层 + 渲染主循环 + 共享模块。
//!
//! 渲染层([`run`] 模块)放在 lib 而不是二进制 crate, 是为了让鸿蒙 staticlib
//! 直接 `use` 它 —— 此前鸿蒙壳只能 `#[path]` include 二进制的 main.rs,
//! 于是要在壳里复制一整份依赖树(并依赖本机 junction, 干净 clone 构建失败)。
//! 放进 lib 之后: 依赖树只有一棵, 资产由 rust-embed 从 `pet/assets/` 直接嵌入。

// Windows GUI 子系统: 不弹控制台窗口(桌宠窗口置顶, 无黑控制台挡道)。
// lib 与二进制 crate 都要声明: 真正生效的是最终链接的那个 crate。
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

pub mod app;
pub mod chat;
pub mod chatlog;
pub mod run;

// 入口 re-export: 桌面二进制与鸿蒙壳都写 `cute_pet::start()`, 不必关心
// 渲染层在哪个模块下。
pub use run::{run, start};
#[cfg(target_env = "ohos")]
pub use run::ohos_set_keyboard_height;

// 平台特有模块(crate 根级, 由 run 使用)。原先声明在 main.rs 里, 随渲染层一起
// 搬进 lib 后必须留在这里 —— 放在 run.rs 内会被解析成 src/run/windows.rs。
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
mod linux;
