# Android WebView 壳原型方案（wasm + webview 跨平台路线）

> 状态：**M1 已落地（Android 壳可构建出 APK），M2–M4 待真机验证**。
> 本文是设计与进度记录；代码在 `pet/webview/`（见 §0）。
> 目标读者：决定要不要把 cute-pet 从「Rust 原生逐平台交叉编译」切到
> 「单 wasm 内核 + 薄原生 WebView 壳」的人。

## 0. 实现位置（M1）

| 文件 | 作用 |
|---|---|
| `pet/webview/android/AndroidManifest.xml` | 壳清单，包名沿用 `rust.cute_pet`（素材目录与原生版重合） |
| `pet/webview/android/src/rust/cute_pet/OverlayService.java` | 悬浮窗 + WebView + 拖动 + 穿透切换 |
| `pet/webview/android/src/rust/cute_pet/PetBridge.java` | JS ↔ Native 桥（§4.3 协议实现） |
| `pet/webview/android/src/rust/cute_pet/MainActivity.java` | 权限引导 + 起服务 |
| `pet/webview/web/index.html` | 透明版 Web 内容层（区别于 `pages/index.html` 的深色底） |
| `pet/webview/web/pet_bridge.js` | 桥的 JS 侧 |
| `pet/webview/android/build.sh` | 纯 SDK 构建（aapt2+javac+d8+zipalign+apksigner，**无 Gradle**） |
| `.github/workflows/android-shell.yml` | 远端构建 + 合规闸门 |

本地：`cd pet/webview/android && bash build.sh` → `bin/cute-pet-shell.apk`。

---

## 1. 为什么要做这件事

当前架构是 Rust（macroquad / ply-engine）**逐平台原生编译**：
桌面三端、Android（NDK + `plyx apk`）、鸿蒙（NAPI）、iOS、WASM（Pages demo）。
这条路线踩到的坑集中在**逐平台适配成本**上：

| 痛点 | 实例（本项目真实发生） |
|---|---|
| 工具链不透传 cargo 参数 | `plyx apk` 只认 `--native/--install/--auto`，导致合规开关 `bundle-murasame` 在 Android 上**无法关闭** → 只能翻转 feature 默认值绕开（v0.3.0） |
| 各平台沙箱路径不一致 | 素材目录要分别为桌面 / Android / 鸿蒙 / iOS 适配（PR #14） |
| 产物分发易漏 | web job 一句 `cp -r assets` 把 117 张版权素材打进 zip（v0.3.0 泄露，v0.3.1 修复） |
| CI 成本 | 8 个平台交叉编译 ~20 分钟，还要维护 NDK r25 / OHOS SDK |

**核心判断**：这些坑里，绝大多数都出在「**逻辑/渲染层**也要跟着每个平台重新编译一遍」上。
如果把这层收敛成一个 wasm 产物，各平台只留一层**薄壳**负责系统级能力，
上面这些成本会大幅下降。

**而且内核已经现成**：Pages demo 的 `app.wasm` + `pages/ply_bundle.js` + `pages/index.html`
就是可运行的内核，实测 **120fps 满帧、渲染正常**。不是从零开始。

---

## 2. 目标与非目标

### 目标（Android 原型，要验证的事）
1. **悬浮窗定位** —— 桌宠悬浮在其他应用之上，可拖动
2. **截屏回传** —— MediaProjection 帧送到 wasm（供 VLM 视觉链路用）
3. **wasm 渲染满帧** —— Android WebView 上达到可用帧率
4. **素材加载** —— 由壳喂给 wasm，让**商业版在 Android 上也有角色**
   （直接解决 v0.3.1 的遗留：商业包不含素材 + Android 拿不到素材）

### 非目标（本轮明确不做）
- ❌ **iOS**：iOS 不允许第三方全局悬浮窗，与 webview 无关，**无解**，不投入
- ❌ **鸿蒙**：待 Android 原型验证后再评估
- ❌ **桌面 Windows 替换**：原生方案已跑通（含屏幕操控/穿透），换壳收益小风险大，暂不动
- ❌ 不追求一次跑通，先立骨架 + Bridge 协议

---

## 3. 架构分层

```
┌─ 原生壳（Java/Kotlin，平台专属，尽量薄）────────────────┐
│  OverlayService        TYPE_APPLICATION_OVERLAY 悬浮窗     │
│  ScreenCaptureService  MediaProjection + VirtualDisplay     │
│  WebView               android.webkit.WebView（内容容器）    │
│  Bridge                addJavascriptInterface / evaluateJs  │
└────────────────────────────────────────────────────────────┘
                    ↕  JSBridge（JSON 消息）
┌─ Web 内容层（HTML/JS，跨平台，只写一次）─────────────────┐
│  pages/index.html + pages/ply_bundle.js                    │
└────────────────────────────────────────────────────────────┘
                    ↕  wasm 导入
┌─ wasm 内核（Rust，单产物）───────────────────────────────┐
│  app.wasm —— 渲染 / 对话 / 状态机 / AI                      │
└────────────────────────────────────────────────────────────┘
```

**能统一的是第 2、3 层；第 1 层省不掉。**
准确说法是「**~70% 代码统一 + 30% 薄原生壳**」，不是零原生。

---

## 4. Android 侧具体设计

### 4.1 复用现有代码

| 现有资产 | 怎么用 |
|---|---|
| `pet/java/OverlayService.java` | **直接复用**。已有 `TYPE_APPLICATION_OVERLAY` + `FLAG_NOT_FOCUSABLE` + `FLAG_NOT_TOUCH_MODAL` + `PixelFormat.TRANSLUCENT` + 拖动 handler（`onDrag` → `wm.updateViewLayout`）。只需把 `wm.addView(view)` 里的 `view` 从 `QuadSurface` 换成 `WebView` |
| `pet/java/ScreenCaptureService.java` | **直接复用**。已有 MediaProjection + VirtualDisplay + ImageReader（~4s 一帧）。只需把帧的去处从原链路改为经 Bridge 送 JS |
| `pages/index.html` + `ply_bundle.js` + `app.wasm` | 作为 Web 内容层，打进 APK assets 或放 app 私有目录 |

### 4.2 悬浮窗 + WebView 关键点

```java
// OverlayService 中改造（示意）
webView = new WebView(this);
webView.setBackgroundColor(Color.TRANSPARENT);   // 关键 1: WebView 透明
webView.getSettings().setJavaScriptEnabled(true);
webView.getSettings().setMediaPlaybackRequiresUserGesture(false);
webView.addJavascriptInterface(new PetBridge(this), "PetBridge");  // 关键 2: Bridge
webView.loadUrl("file:///android_asset/pet/index.html");

wm.addView(webView, params);   // params 沿用现有（含拖动 handler）
```

**三个必须处理好的点：**

1. **透明**：`setBackgroundColor(Color.TRANSPARENT)` + 页面侧 `body/canvas` 背景透明。
   现有 `pages/index.html` 是 `background: #0e0e12`（深色不透明），需要一版透明变体。
2. **点击穿透 vs 可交互**：现有 `FLAG_NOT_FOCUSABLE` 让触摸穿透到下层应用，
   但桌宠需要点击互动。需要按角色像素做「透明区穿透 / 角色区接收」——
   这是 Android 悬浮窗的经典难题，本轮需明确取舍（见 §7 风险）。
3. **拖动**：现有 `onDrag` 通过 `wm.updateViewLayout` 移动窗口，
   与 WebView 共存时要确认触摸事件不被 WebView 吞掉。

### 4.3 Bridge 协议（JSON，双向）

**JS → Native**（`PetBridge.postMessage(json)`）
| 方法 | 用途 |
|---|---|
| `overlay.move(dx, dy)` | 移动悬浮窗 |
| `overlay.setSize(w, h)` / `setFullscreen(bool)` | 尺寸 / 全屏 |
| `capture.request()` | 请求一帧屏幕 |
| `clipboard.get()` / `clipboard.set(text)` | 剪贴板 |
| `keyboard.show(bool)` | 软键盘 |
| **`asset.fetch(path)`** | **向壳索取素材（角色素材不进 wasm，见 §5）** |

**Native → JS**（`webView.evaluateJavascript("PetNative.onMessage(...)")`)
| 事件 | 内容 |
|---|---|
| `capture.frame` | 截屏帧（base64 + 时间戳 + 尺寸） |
| `asset.data` | `asset.fetch` 的返回（base64） |
| `window.state` | 窗口尺寸/位置/全屏变化 |
| `lifecycle` | resume / pause / MediaProjection `onStop`（Android 16 会主动停） |

### 4.4 截屏回传的带宽注意

MediaProjection 全屏帧经 base64 走 JSBridge **开销很大**。建议：
- 降采样（如长边 ≤ 720）+ JPEG 质量 0.7
- 或 Native 侧写入 app 私有文件，只回传 `file://` 路径由 JS fetch
- 保持现有 ~4s 间隔（VLM 视觉链路不需要更高频）

---

## 5. 素材加载：顺带解决合规遗留

这是本方案**额外收获**最大的一块。

- v0.3.1 起商业包不含角色素材（合规要求，见 `THIRD-PARTY-NOTICES.md` §2.2）
- 桌面端靠外部目录（PR #14：exe 同级 `assets/` 或 `PET_ASSETS_DIR`）
- **Android / 鸿蒙 / WASM 拿不到素材** → 商业版在这些平台没角色

走 WebView 壳后，统一由 `asset.fetch(path)` 让**壳**提供素材：
- Android：壳从 app 私有目录 / `/sdcard/Android/data/<pkg>/files/assets` 读
- WASM：壳（部署方）从同源 HTTP 提供
- 桌面：壳从本地目录读

**结果**：`external_asset_dirs()` 那套逐平台路径适配可以被 Bridge 取代，
wasm 侧只剩一句「向壳要素材」，各平台差异收敛到壳里。
合规闸门（发布前扫描产物无 `murasame*`）继续生效 —— **壳提供的素材由用户自备，不进分发产物**。

---

## 6. 验证清单（原型是否成立的判据）

| # | 验证项 | 通过标准 | 现状 |
|---|---|---|---|
| 1 | 悬浮窗定位 | 桌宠浮在其他应用之上，可拖动，位置持久化 | 代码就绪，待真机 |
| 2 | 透明背景 | 桌宠以外区域能看到下层应用（不是黑/白块） | 代码就绪，待真机 |
| 3 | 截屏回传 | `capture.request()` → wasm 收到帧，端到端 < 1s | M3 |
| 4 | wasm 渲染 | Android WebView 上 ≥ 45fps（桌面实测 120fps） | 待真机 |
| 5 | 素材加载 | `asset.fetch` 拿到角色素材，商业版能显示角色 | 桥已通，Rust 侧对接 M3 |
| 6 | 音频 | 桌宠语音能播（Android WebView 音频策略需验证） | 待真机 |
| 7 | 体积/启动 | APK 体积与启动时间与现有原生版对比（需给出数据） | 体积✅ 启动待真机 |

**第 7 项必须有数据** —— 这是决定要不要切换的关键，不能只凭感觉。

### 6.1 已有实测数据（CI，2026-09-21）

| 项 | 数值 |
|---|---|
| 壳 APK（商业版 wasm，debug 签名） | **1.3 MB**（du 按 1K 块）/ 未压缩 5.8 MB |
| `app.wasm`（商业版，无角色素材） | 2.7 MB |
| `ply_bundle.js` / `pet_bridge.js` / `index.html` | 48K / 8K / 4K |
| `classes.dex`（壳全部 Java，含框架引用） | ~24 KB |
| 合规闸门 | ✅ 产物无 `*murasame*` |
| 构建耗时 | 约 1m20s（含 wasm 编译；rust-cache 命中时更快） |

对照：原生 Android 版用的是同一份 wasm 逻辑的原生交叉编译产物 + `QuadSurface`。
**APK 体积的差异主要来自 wasm 内核本身**（2.7MB），Java 壳只有 24KB ——
这正是「薄壳」的预期形态。启动时间必须真机测，wasm 首次实例化 + 字体解析是主要变数。

---

## 7. 风险与已知限制

| 风险 | 说明 | 缓解 |
|---|---|---|
| **iOS 无解** | iOS 不允许第三方全局悬浮窗，与架构无关 | 不投入；现有 iOS 产物同样受此限制 |
| 透明 WebView 兼容性 | 部分 ROM / WebView 版本透明渲染异常 | 原型阶段在多台设备验证 |
| 点击穿透 vs 交互 | `FLAG_NOT_FOCUSABLE` 与「点击角色互动」冲突 | 需设计按区域/按像素的方案，或做成可切换模式 |
| 性能 | Android WebView 跑 wasm 未必达到桌面水平 | 第 4 项实测；必要时降分辨率/关特效 |
| IME / 软键盘 | 悬浮窗 WebView 里输入框调键盘行为待验证 | `keyboard.show` + WebView `focus` 实测 |
| 截屏权限体验 | MediaProjection 每次需用户授权 | 复用现有 ScreenCaptureService 的前台服务与通知 |
| Android 16 限制 | 锁屏/来电会主动停止 MediaProjection | 现有 ScreenCaptureService 已注册 `Callback.onStop` |

---

## 8. 里程碑

- **M1 壳与内核跑通** —— ✅ 代码已落地并可出包（apk 3.5M，dex/内容层齐全，CI 可构建）；
  ⏳ 仍缺**真机**确认：透明渲染、帧率、拖动手感
- **M2 Bridge 接通** —— ✅ move / setSize / setPassthrough / clipboard / keyboard / log 已实现；
  ⏳ 待真机联调
- **M3 截屏 + 素材** —— 桥的 `asset.fetch` 已通（Rust 侧对接未做）；`capture.request` 待接
  `pet/java/ScreenCaptureService.java`
- **M4 评估决策**：与现有原生 Android 构建对比（体积 / 启动 / 帧率 / 维护成本），
  数据化决定是否切换 —— **必须先有真机数据**

### 8.1 构建方式的一个取舍：不用 Gradle

壳只用 framework API，不引 androidx，所以能用 SDK 自带的 aapt2/javac/d8/zipalign/apksigner
直接出包，**CI 无需联网拉依赖**。代价是没有 Gradle 的资源合并与多渠道；
将来若引入 XML 布局或第三方库，再迁 Gradle 不迟。

踩过的三个坑（都已在 `build.sh` 里修掉，记下来免得重犯）：

1. Git Bash 下 aapt2/javac/d8/zipalign/keytool 都是**原生 Windows 程序**，
   POSIX 路径（`/d/...`）会被当成 `\d\...`（盘符相对路径），凡绝对路径都要
   `cygpath -w` 转一遍。javac 的 `@argfile` 内容同理。
2. `winpath()` 的非 Windows 分支若用 `printf '%s'`（不带换行），
   生成 argfile 时所有路径会**粘成一行**，Linux CI 上表现为
   `file not found: a.javab.javac.java`。两个分支都必须输出换行。
3. `--out` 传相对路径时（CI 就是 `--out dist-shell`），打包那步的
   `(cd "$out_dir/assets" && zip ... "$APK")` 会因为 `cd` 改了 cwd 而找不到
   目标 APK。参数一律先转绝对路径。

---

## 9. 关于 webview-c / webview-mini（已调研，结论存档）

用户自有仓库 `webview-capi` / `webview-mini` / `xmake-mirror` 的实测结论
（2026-09-21 查证）：

- **只有 Windows 是实的**：`webview-capi` 仓库内仅有 `webview.dll` / `webview.lib`
  （Windows 预编译产物）；`webview-mini` 是 Windows WebView2 单头文件封装
- README 声称「Windows / macOS / Linux 透明工作」，但
  `cross-platform.yml` 的 job 指向**已不存在的目录** `lyco-webview/lyco`，
  `android.yml` 的 `cd android` 与实际 `demo/android/` 不符，
  **最近 8 次 CI 全部 failure**
- `demo/rust-ffi/src/lib.rs` 是占位实现（"Placeholder - actual implementation in C"）
- 无 iOS / 鸿蒙

**分工结论**：webview-c 能复用的是 **API 形态**（单头文件 C API + Rust FFI 占位）
和 **Windows 侧现成产物**；**Android 必须用系统 `android.webkit.WebView` 另写 Java 壳**，
webview-c 在 Android 上用不了。

若将来要统一桌面 Windows 壳，webview-mini（200KB exe）是合适的起点，
但需自行补**透明 + 穿透 + 置顶**的平台代码（标准 webview C API 不提供这些）。

---

## 10. 一句话总结

> 走 wasm + WebView 能砍掉的是「逻辑/渲染层的多平台编译与适配」，
> 砍不掉的是「悬浮窗 / 截屏 / 前台服务」这些系统能力 —— 后者继续用你已有的
> `OverlayService` / `ScreenCaptureService`，只是内容区从 `QuadSurface` 换成 `WebView`。
> Android 是收益最大的第一站（正好绕开 plyx / NDK / 沙箱路径这些坑），
> 但**必须用 Java 壳，webview-c 帮不上**；iOS 无解，不要投入。
