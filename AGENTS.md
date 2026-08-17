# cute_box 项目 — Agent 引导

本仓库 `D:\Code\cute_box\` 含多个子项目:
- **`pet/`** — 跨平台桌宠「丛雨」(Rust + 宏 quad + miniquad-ply), 本仓库核心
- **`lazy-ply/`** — 自研 UI 组件库(被 pet 依赖)
- **`lyco-chat/`**, **`lyco-engine/`** — 相关预研/引擎

## 2. 关键文档(动手前先读)

| 方向 | 文档 |
|---|---|
| **鸿蒙开发手册(agent 可执行)** | [`docs/HARMONYOS_RUST_AGENT.md`](docs/HARMONYOS_RUST_AGENT.md) |
| 鸿蒙坑详解 | `pet/docs/harmonyos-rust.md` |
| 项目进度/环境备忘 | `progress.md` |
| GDI 桌面低内存版 | `pet/gdi_desktop/`(独立小项目) |

## 3. 高频须知

- **鸿蒙交叉编译**: `target_os="linux"` 但 `target_env="ohos"`; 所有 Linux 专属 cfg 必须排除 ohos。
  命令: `cargo build --target aarch64-unknown-linux-ohos --release`(target 在末尾)。
  首次/换 SDK 先跑 `bash pet/tools/init_ohos_rt.sh` 补 CRT。
- **miniquad-ply / quad-snd 是本地 vendored 版**(`pet/vendor/`), 由 `pet/Cargo.toml` 的
  `[patch.crates-io]` 接管; 修改它们会全局影响所有平台。
- **用户偏好**: 简短扼要汇报、先验证再写、高内聚低耦合、分步验证(先基本形状再复杂)、
  不要用会话 cookie 发外部网络请求。

## 4. 用法示例

```bash
# 本地运行桌宠(桌面)
cd pet && cargo run
# 鸿蒙交叉编译
cd pet && bash tools/init_ohos_rt.sh && cargo build --target aarch64-unknown-linux-ohos --release
```

> 遇到工作流/规格/前沿问题, 可参考 `lyco` skill(预研先行) 与 `oma-*` 系列 skill。
