//! cute-pet 库 —— 桌宠内核, 按域分层, 供桌面二进制 / 鸿蒙 staticlib / WebView 壳共用。
//!
//! 分层(2026-09-23 重构, 高内聚低耦合, 方便外部组织集成):
//! - [`chat`]:     对话域 —— 台词生成 + 语气学习(可独立测试, 不依赖渲染)
//! - [`app`]:      应用状态层 —— 事件驱动状态机(update 为纯函数)
//! - `assets`:     资产域 —— 嵌入资产注册表 + manifest 模型 + 合规断言(内部)
//! - `voice`:      语音域 —— 台词-语音-表情映射与播放(内部)
//! - `ui`:         表现层 —— 界面文案 / 待机动画 / 界面部件(内部)
//! - [`run`]:      装配层 —— 窗口配置 + 主装配与主循环(唯一渲染入口)
//! - `platform`:   平台差异层 —— 各 OS 专属实现, 编译期 cfg 选择(内部)
//!
//! 外部集成方式(参照 pet/host 鸿蒙壳): 只依赖 `cute_pet::start()`
//! (鸿蒙另需 `ohos_set_keyboard_height`), 不触碰内部模块 ——
//! 依赖树只有一棵, 资产由 rust-embed 从 `pet/assets/` 直接嵌入。

// Windows GUI 子系统: 不弹控制台窗口(桌宠窗口置顶, 无黑控制台挡道)。
// lib 与二进制 crate 都要声明: 真正生效的是最终链接的那个 crate。
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

// ---- 对集成者公开的模块 ----
pub mod app;
pub mod chat;
pub mod run;

// 入口 re-export: 桌面二进制与鸿蒙壳都写 `cute_pet::start()`, 不必关心
// 渲染层在哪个模块下。
pub use run::{run, start};
#[cfg(target_env = "ohos")]
pub use run::ohos_set_keyboard_height;

// 兼容旧路径 `crate::chatlog`(语气学习可独立使用)。
pub use chat::chatlog;

// ---- 内部域模块: 集成者无需直接依赖, 符号可见性收敛在 crate 内 ----
pub(crate) mod assets;
pub(crate) mod platform;
pub(crate) mod ui;
pub(crate) mod voice;
