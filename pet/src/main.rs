//! 桌面/移动二进制入口: 只做装配, 渲染逻辑全在 [`cute_pet::run`]。
//!
//! 鸿蒙走 pet/host 的 staticlib, 调的是同一个 `cute_pet::start()`,
//! 两边不会再编出不同的窗口配置或依赖树。

// Windows GUI 子系统: 不弹控制台窗口(桌宠窗口置顶, 无黑控制台挡道)。
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    cute_pet::start();
}
