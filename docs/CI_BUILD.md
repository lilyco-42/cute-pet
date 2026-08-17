# 8 平台 CI 构建 + 自动发布 — 方案与踩坑手册

> 桌宠(丛雨)全平台持续构建: 一次 push → 13 个构建 job → 全绿自动打 tag + 发 GitHub Release。
> workflow: `.github/workflows/build-all.yml`(唯一入口, 自包含, 无外部复用)。

## 0. 一句话结论

Android(3 ABI) · Windows(x64/arm64) · Linux(x64/arm64) · macOS(arm64/x64) ·
iOS(device/sim) · Web(WASM) · HarmonyOS(aarch64) **共 13 个构建 job 全部通过**,
产物自动打包上传到 [GitHub Releases](https://github.com/lilyco-42/cute-pet/releases)。
当前已发布: **cute-pet v0.1.1**(13 个平台 zip)。

## 1. 触发与总览

```yaml
on:
  push:      { branches: [main, master] }
  pull_request: { branches: [main, master] }
  workflow_dispatch:
```

- 每个平台一个 job, `strategy.matrix` 展开 ABI/架构, `fail-fast: false`(互不拖累)
- 关键约定: **所有 `run` 步骤 `defaults.run.working-directory: pet`**(Cargo.toml 在 `pet/` 子目录)
- 上传产物用 `upload-artifact@v4`; **path 一律写 `pet/...` 前缀**(相对仓库根, 不是 working-directory)

## 2. 平台 job 速查

| Job | runner | 构建方式 | 产物 |
|---|---|---|---|
| android | ubuntu | `plyx apk --native`(需 NDK r25) | 3 × APK |
| windows | windows-latest | `cargo build --target ...` | cute-pet.exe |
| linux | ubuntu | `cargo build --target ...`(aarch64 需交叉依赖) | ELF |
| macos | macos-latest | `cargo build --target ...` | 二进制 |
| ios | macos-latest | `cargo build --target ...` + 手搓 .app | .app bundle |
| web | ubuntu | `cargo build --profile release-wasm` | app.wasm + assets |
| harmony | ubuntu | RUSTFLAGS 覆盖 sysroot + clang | aarch64 ELF |

## 3. 构建缓存

全部 job 缓存 `~/.cargo/registry` + `~/.cargo/git`(key: `cargo-<os>-<Cargo.lock hash>`);
除 Android 外的矩阵 job 额外缓存 `pet/target`(key: `target-<os>-<target>-<lock hash>`,
restore-keys 逐级回退)。Web 单独固定 key(`wasm32-unknown-unknown`)。
注意: **不要用 YAML anchor**(`&cache` / `<<: *cache`)——actionlint 与 GitHub 解析器
对 anchor 支持不一致, 直接内联最稳。

## 4. 自动发布(release job)

```
全部平台 job 全绿 → release job(if: main 分支 push)
  → download-artifact 全部产物到 dist/
  → 每个 artifact 目录 zip 打包(避免同名文件冲突, 平台隔离)
  → 读 pet/Cargo.toml 版本号 → tag = v<版本>
  → gh release create(gh release view 已存在则跳过)
```

- `permissions: contents: write`(发布必需)
- tag 已存在 → `exit 0` 跳过(同版本重复 push 不报错)
- notes 固定列出 7 平台清单(可后续丰富为自动 changelog)

## 5. 踩坑记录(按平台)

### 5.1 通用: 上传路径少 `pet/` 前缀
`upload-artifact` 的 `path` 相对 **仓库根**(不是 `defaults.run.working-directory`), 写成
`target/...` 永远匹配不到(项目在 `pet/` 下)。后果: artifact 静默为空(`if-no-files-found: warn`
不报错), 发布 zip 也空。**实测只修到 Android 一个平台有内容**(它 path 写了 `pet/target/...`)。
修复: 所有 upload path 加 `pet/` 前缀。

### 5.2 Linux x86_64: `-lasound` 链接失败
`quad-snd`(vendored 音频)依赖 ALSA, CI 缺 `libasound2-dev`。
修复: apt 装 `libasound2-dev`。

### 5.3 Linux aarch64: 交叉编译三步坑
1. `ring` 报 `ToolNotFound: aarch64-linux-gnu-gcc` → apt 装 `gcc-aarch64-linux-gnu`,
   并设 `CC_aarch64_unknown_linux_gnu`。
2. 链接阶段 `rust-lld` 报 `symbols.o incompatible with elf64-x86-64` → 设
   `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc`。
3. **apt 源坑**(最难): GitHub runner 的 host 源(`azure.archive.ubuntu.com` /
   `security.ubuntu.com`) **没有 arm64 Packages**, `dpkg --add-architecture arm64` 后
   `apt-get update` 直接 404 失败。
   修复: `sed` 给 `ubuntu.sources` 每节加 `Architectures: amd64`(host 源只拉 amd64),
   另写 `/etc/apt/sources.list.d/arm64-cross.list` 指向 `http://ports.ubuntu.com/ubuntu-ports/`
   (noble / noble-updates / noble-security, 带 `[arch=arm64]`), 再装
   `libx11-dev:arm64` 等全部 `:arm64` 开发库 + `PKG_CONFIG_ALLOW_CROSS=1`。

### 5.4 macOS: `_objc_msgSend_ret` 链接失败(两次误诊)
症状: `Undefined symbols: "_objc_msgSend_ret", referenced from cute_pet...`。
- 第一轮误诊: 以为是 deployment target 太低 → `MACOSX_DEPLOYMENT_TARGET=13.0`
  无效(`-mmacosx-version-min=13.0.0` 已生效仍失败)。
- **真根因**: `pet/src/macos.rs` 的 `extern "C"` 块自己声明了 `fn objc_msgSend_ret(...)`,
  但 **libobjc 根本没有导出这个符号**(它不是 Apple 运行时符号)。
  修复: 删除该声明, `styleMask`(NSUInteger 标量返回)直接复用 `objc_msgSend`
  (arm64/x86_64 上标量与指针返回走同一返回寄存器, 调用处 `as isize` cast)。
- 保留 `MACOSX_DEPLOYMENT_TARGET=14.0`(无害, 桌宠不依赖旧系统)。
- 教训: 先搜代码里有没有自己声明该符号, 再怀疑工具链。

### 5.5 HarmonyOS: CRT 复制"same file"崩溃
`setup-ohos-sdk@v1.0.1` 拉 SDK 6.1 后, **sysroot 已自带 `libunwind.a`**(旧版没有),
`cp` 同路径报 `are the same file` → step 失败(间歇性, 取决于 SDK cache 新旧)。
修复: CRT 复制幂等化(`[ -f "$DEST/xxx" ] || cp ...`), crtbegin/crtend 同理。

### 5.6 Android: 两个坑
1. `plyx` 报 `Android NDK r25 not found` → `setup-android@v3` 不装 NDK;
   补 `sdkmanager "ndk;25.2.9519653" --sdk_root=$ANDROID_HOME`。
2. `aapt2: No such file or directory: .../assets_apk/.` → Cargo.toml 里
   `[package.metadata.android] assets = "assets_apk/"` 引用的目录未提交(git 不跟踪空目录)。
   修复: 提交 `pet/assets_apk/.gitkeep`。
3. Android 不缓存 `pet/target`(plyx 在 `/tmp/plyx-apk-native` 独立构建, 缓存无意义)。

### 5.7 iOS: `dtolnay/rust-toolchain` 429
瞬时 GitHub 限流(`Failed to download action ... 429 Too Many Requests`), 重跑即过, 无代码修复。
iOS 的 .app 是 run 步骤手搓(Info.plist heredoc), `UILaunchStoryboardName` 留空。

## 6. 本地构建命令对照

| 平台 | 命令(在 `pet/` 下) |
|---|---|
| 桌面(Windows) | `cargo run` |
| Linux/macOS/Windows | `cargo build --release --target <target>` |
| Web | `cargo build --target wasm32-unknown-unknown --profile release-wasm` |
| Android | `cargo install plyx && plyx apk --native`(先 sed 改 build_targets) |
| HarmonyOS | 见 `docs/HARMONYOS_RUST_AGENT.md`(CC/AR/CFLAGS 环境变量 + init_ohos_rt.sh) |

## 7. 发布产物清单(v0.1.1)

13 个 zip ≈ 5-10MB 每个: android ×3(APK) / windows ×2(exe) / linux ×2(ELF) /
macos ×2 / ios ×2(.app) / web(wasm+assets 10MB) / harmony(ELF 5.6MB)。
