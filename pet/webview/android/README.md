# Android WebView 悬浮窗壳（M1）

> 方案文档：[`docs/webview-shell-android.md`](../../../docs/webview-shell-android.md)
> 一句话：**悬浮窗骨架沿用原生版，内容区从 `QuadSurface` 换成 `WebView`，里面跑 wasm 内核。**

## 为什么没有 Gradle

这个壳**只用 Android framework API**（`android.webkit.WebView` / `WindowManager` 悬浮窗 /
`Notification` / `ClipboardManager`），不引 androidx，也没有任何第三方依赖。
所以直接用 SDK 自带工具链就能出包，**不需要 Gradle，也不需要联网拉依赖**：

```
aapt2 link  →  javac  →  d8  →  zip(assets+dex)  →  zipalign  →  apksigner
```

代价是没有 Gradle 的依赖管理、资源合并与多渠道。本壳零 res 目录、UI 全在代码里搭，
目前够用；将来若引入 androidx 或 XML 布局，再换 Gradle 不迟。

## 目录

```
pet/webview/android/
├── AndroidManifest.xml                 壳的清单(包名沿用 rust.cute_pet)
├── build.sh                            纯 SDK 构建脚本(Windows / Linux 均可)
├── src/rust/cute_pet/
│   ├── MainActivity.java               入口: 申请悬浮窗权限 + 起服务
│   ├── OverlayService.java             悬浮窗 + WebView + 拖动 + 穿透切换
│   └── PetBridge.java                  JS <-> Native 桥(协议实现)
└── assets/pet/                         内容层(由 build.sh 组装, 不入库)
    ├── index.html                      透明版页面(区别于 pages/index.html)
    ├── pet_bridge.js                   JS 侧桥
    ├── ply_bundle.js                   ply-engine web 引导包(从 pages/ 拷)
    └── app.wasm                        wasm 内核(构建产物)
```

Web 内容层源码在 [`pet/webview/web/`](../web/)（透明 `index.html` + `pet_bridge.js`）。

## 构建

```bash
# 商业版(默认): wasm 不含第三方角色素材
cd pet && cargo build --target wasm32-unknown-unknown --profile release-wasm
cd ../pet/webview/android && bash build.sh

# 自测版(个人真机调试用, 产物不得发布):
cd pet && cargo build --target wasm32-unknown-unknown --profile release-wasm --features bundle-murasame
cd ../pet/webview/android && bash build.sh

# 指定 wasm / 输出目录
bash build.sh --wasm /path/to/cute-pet.wasm --out /tmp/shell
```

产物：`bin/cute-pet-shell.apk`（调试签名）。远端构建走
[`.github/workflows/android-shell.yml`](../../../.github/workflows/android-shell.yml)。

## 安装与验证

```bash
adb install -r bin/cute-pet-shell.apk
adb shell appops set rust.cute_pet SYSTEM_ALERT_WINDOW allow   # 免手动点设置页
adb logcat -s CutePetOverlay PetBridge
```

验证清单（详见方案文档 §6）：

| # | 项 | M1 状态 |
|---|---|---|
| 1 | 悬浮窗定位 + 拖动 | ✅ 已实现（拖动阈值 10dp，未达阈值转发给 JS 当点击） |
| 2 | 透明背景 | ✅ 已处理（WebView 透明 + 页面透明 + Rust 侧 `clear_background(0,0,0,0)`）——**待真机确认** |
| 3 | wasm 渲染 | ⏳ 待真机（桌面 Pages 实测 120fps） |
| 4 | 穿透 / 交互切换 | ✅ `PetBridge.setPassthrough` 已实现 |
| 5 | 素材 `asset.fetch` | ✅ 桥已通（Rust 侧对接在 M3） |
| 6 | 截屏回传 | ❌ M3 |
| 7 | 体积 / 启动数据 | ⏳ CI 已打印体积，启动时间待真机 |

## 合规

- 默认构建 **不含**第三方版权角色素材（见 `THIRD-PARTY-NOTICES.md` §2.2）。
- 角色素材由用户自备，放在 `/sdcard/Android/data/rust.cute_pet/files/assets/`
  —— 与原生版 `external_asset_dirs()` 的 Android 分支**完全重合**，放一份两边都能用。
- CI 有闸门：默认构建的 APK 里出现任何 `*murasame*` 文件即失败。
