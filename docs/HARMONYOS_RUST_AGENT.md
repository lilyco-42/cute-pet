# Rust 开发 HarmonyOS 桌宠 — Agent 操作指南

> **让其它 agent 接手本任务的完整手册**。项目: `D:\Code\cute_box\pet`(桌宠, 宏 quad + miniquad-ply)。
> 本指南记录: 环境、可执行命令、所有坑的解法、当前状态、下一步。

---

## 0. 一句话结论

鸿蒙 Rust 支持是官方 tier-2(`aarch64-unknown-linux-ohos`), 桌宠已能完整交叉编译出原生 ELF。
**核心陷阱**: 鸿蒙靶的 `target_os = "linux"` 而 `target_env = "ohos"` —— 所有 Linux 专属代码会在鸿蒙误触发。

## 1. 环境(本机已配好, 勿重配)

| 项 | 位置 |
|---|---|
| Rust ohos target | `rustup target add aarch64-unknown-linux-ohos`(已装) |
| 编译器 clang | `D:\app\scoop\apps\llvm\current\bin\clang.exe`(LLVM 22) |
| OHOS SDK(sysroot/运行时) | `D:\ohos-sdk\command-line-tools\` |
| 项目级配置 | `.cargo/config.toml`(linker=clang + sysroot + CC/CFLAGS) |
| vendor 依赖 | `vendor/miniquad-ply/`(ohos 后端), `vendor/quad-snd/`(ohos 音频桩) |

## 2. 构建命令(可直接跑)

```bash
cd D:/Code/cute_box/pet
# 0) CC/AR/CFLAGS 必须由环境提供(见 .cargo/config.toml 注释)
export CC_aarch64_unknown_linux_ohos=D:/app/scoop/apps/llvm/current/bin/clang
export AR_aarch64_unknown_linux_ohos=D:/app/scoop/apps/llvm/current/bin/llvm-ar
export CFLAGS_aarch64_unknown_linux_ohos="--target=aarch64-linux-ohos --sysroot=D:/ohos-sdk/command-line-tools/sdk/default/openharmony/native/sysroot"
# 1) 补 CRT(首次或 SDK 变了; 幂等)
bash tools/init_ohos_rt.sh
# 2) 交叉编译(注意: --target 在末尾)
cargo build --target aarch64-unknown-linux-ohos --release
# 产物:
#   target/aarch64-unknown-linux-ohos/release/cute-pet  (11.8MB ELF aarch64 PIE)
```

**必须记得**:
- `--target aarch64-unknown-linux-ohos` 要放在 build 末尾(不是 cargo 后面)
- **CC/AR/CFLAGS 是硬性必需**(ring 等 C 依赖靠它们交叉编译); 缺失会报
  "ring ToolNotFound cc"。这些是外部环境变量, 不写在 config 的 `[env]` 表(那条不展开)。

## 3. 本项目为鸿蒙做了什么(架构)

```
pet/
├─ .cargo/config.toml          # ohos linker/sysroot 配置(本地 Windows 死路径; CI 用 RUSTFLAGS 覆盖)
├─ Cargo.toml                  # [patch.crates-io] 指向本地 vendor
├─ vendor/miniquad-ply/        # 仿 android 的 ohos 原生后端
│   └─ src/native/ohos.rs      #   EGL 渲染线程 + ohos_surface_* 导出符号
├─ vendor/quad-snd/            # 鸿蒙 noop 音频桩(结构对齐 alsa)
│   └─ src/ohos_snd.rs
├─ tools/init_ohos_rt.sh       # 复制 NDK clang runtime 到 sysroot(修复 CRT)
├─ docs/harmonyos-rust.md      # 详细坑清单
└─ .github/workflows/build-all.yml  # 8 平台 CI(含 harmony job)
```

### ⚠️ cargo config 的 `${VAR}` 局限(重要, 易踩)
cargo `.cargo/config.toml` 的 `${VAR}` 展开**只认内置 config 变量**(如 `$CARGO_HOME`),
**不引用任意环境变量**; `[env]` 表同样不展开 `${VAR}`。所以:
- **本地(Windows)**: sysroot 路径写死 `D:/...`(能构建), 见 .cargo/config.toml
- **CI(Linux)**: 用 `RUSTFLAGS` 环境变量覆盖 config 的 rustflags —— 见 workflow harmony job 的
  "Set ohos cross env" 步(它把 `RUSTFLAGS`/`CC_*`/`CFLAGS_*` 写进 `$GITHUB_ENV`)
- CC/AR/CFLAGS 必须由 shell 环境变量提供(不是 `[env]` 表)

### miniquad-ply ohos 后端(native/ohos.rs)
仿 `native/android.rs`:
- 独立渲染线程: `libEGL` 动态加载 → `create_egl_context` → `eglCreateWindowSurface(OHNativeWindow)` → 渲染循环(`frame`: update+draw+swap)
- 导出符号供 ArkTS/NAPI 调用: `ohos_surface_created`(传 OHNativeWindow*)、`ohos_surface_changed(w,h)`、`ohos_surface_destroyed`、`ohos_touch`、`ohos_char`、`ohos_pause`/`ohos_resume`
- 事件经 mpsc 从主线程投递到渲染线程(与 android 同构)

## 4. 踩过的坑(按时间序, 全部已解决)

### 坑1: `target_os="ohos"` 编译不过
- `rustc --print cfg --target aarch64-unknown-linux-ohos` 显示它其实是 **`target_os="linux"` + `target_env="ohos"`**
- 所有 `#[cfg(target_os="ohos")]` 都要写成 `#[cfg(target_env="ohos")]`
- 涉及: miniquad-ply 的 egl.rs/module.rs/native.rs/lib.rs + 本项目 main.rs/linux.rs

### 坑2: 链接报 `-lX11` / `-lasound`
- **根因**: `target_os="linux"` 让 miniquad 的 linux_x11/wayland、quad-snd 的 alsa、
  本项目的 `src/linux.rs`(X11 置顶) 全被编译并链接不存在的系统库
- **解法**: 所有 Linux 专属都排除 ohos:
  ```rust
  #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
  ```
  逐处:`vendor/miniquad-ply/src/*.rs` + `pet/src/linux.rs` + `pet/src/main.rs` 的调用点

### 坑3: 链接报 `cannot open crtbeginS.o` / `-lunwind`
- **根因**: OHOS SDK 的 clang runtime 命名是 `clang_rt.crtbegin.o/crtend.o`, libunwind
  在 `llvm/lib/<arch>/libunwind.a`, 都不在 clang 默认搜索路径
- **解法**: 复制成默认名放 sysroot(见 tools/init_ohos_rt.sh):
  ```
  clang_rt.crtbegin.o → sysroot/usr/lib/aarch64-linux-ohos/crtbeginS.o
  clang_rt.crtend.o   → 同目录/crtendS.o
  llvm/lib/aarch64-linux-ohos/libunwind.a → 同目录/libunwind.a
  ```

### 坑4: `ring`(ureq→rustls) 编译 "ToolNotFound: cc"
- **解法**: config 里设 `CC_aarch64_unknown_linux_ohos` + `CFLAGS_aarch64_unknown_linux_ohos`
  指向 clang + `--target=aarch64-linux-ohos --sysroot=...`

### 坑5: fontdb 拉 fontconfig 导致 `-lX11` 残留
- **根因**: 关了 miniquad 的 X11 后仍报 `-lX11` → 来自 lazy-ply 的 `fontdb` 默认 feature `fontconfig`
- **解法**: 本项目字体全 rust-embed 内嵌(`font_wenkai.ttf`), 不查系统字体 → 全平台关掉 fontconfig:
  ```toml
  fontdb = { version = "0.16", default-features = false, features = ["fs","memmap","std"] }
  ```

### 坑6: quad-snd 音频桩结构
- alsa_snd 用 `pub use crate::mixer::Playback`(不自定义), `AudioContext::new()->AudioContext`,
  `Sound{ sound_id }` + `Sound::load/play/delete`
- ohos_snd.rs 必须结构对齐(`mixer_ctrl` 字段 + `pub use mixer::Playback`), 否则 E0308

## 5. 当前状态(2026-08 验证)

- ✅ ohos 交叉编译 + 链接成功, 产出原生 ELF `target/aarch64-unknown-linux-ohos/release/cute-pet`
- ✅ 其它平台回归: Windows / WASM / Linux 构建通过, 12 单测通过
- ✅ 8 平台 GitHub Action: `.github/workflows/build-all.yml`
- ⏳ 待办: ArkTS 壳(HAR/NAPI) — 用 XComponent 承载 EGL surface, 把 ohos_surface_* 事件接进来;
  鸿蒙音频用 OH_AudioRenderer(替代 noop 桩)

## 6. 下一步(给接手 agent)

1. **建 ArkTS 壳**: DevEco 工程里加 XComponent, `onSurfaceCreated` 拿 OHNativeWindow
   传给 `ohos_surface_created`; size/touch/key/pause 事件转发到对应 ohos_* 导出
2. **打包**: 把 cute-pet ELF 作为 native lib(或通过 ndk-rs/napi)打进 HAR
3. **真机验证**: 用 hdc 推送到鸿蒙设备跑, 确认渲染循环
4. 参考: [RustDesk→鸿蒙移植](https://blog.csdn.net/COLLINSXU/article/details/161957738)、
   [ohos-rs(napi-rs fork)](https://github.com/tdcare/ohos-rs)、
   [Rust 官方 openharmony.md](https://android.googlesource.com/toolchain/rustc/+/refs/heads/master/src/doc/rustc/src/platform-support/openharmony.md)

## 7. 安全提示
- 华为开发者站下载/cookie 是登录凭据, 不要用会话 cookie 发外部请求; SDK 可直接下载或走 GitHub Action 的 `OHOS_SDK_URL` secret
