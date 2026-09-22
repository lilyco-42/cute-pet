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

### 4.4 截屏回传（两种粒度，分两阶段）

**M3-part1（已落地）：截 WebView 自身画面。**
`capture.request` 把 WebView 临时切到 `LAYER_TYPE_SOFTWARE` 后 `draw(Canvas)` 到 Bitmap
（即桌宠自己的渲染画面，透明通道保留），压 PNG(base64) 经 `capture.frame` 推回 JS。
> 注：原本想用 `PixelCopy`，但实测 CI 平台（android-37.2-beta3）的 `PixelCopy` 只有
> Surface/SurfaceView/Window 重载、**没有 View 重载**，WebView 不能直接传。软件层 draw 全 SDK 可用、能保留透明，
> 对小尺寸桌宠一次性截屏足够。开销小、无需用户授权、无 MediaProjection 的 Android 16 停服问题。用途：分享/存档桌宠截图。

**M3-part2（待做）：全屏感知喂 VLM。**
若要让宠物"看见"屏幕（视觉链路），才需要 `MediaProjection` 全屏帧。那套带宽注意如下，
且要复用 `pet/java/ScreenCaptureService.java` 的前台服务/通知/Android 16 `onStop` 处理：

> MediaProjection 全屏帧经 base64 走 JSBridge **开销很大**。建议：
> - 降采样（如长边 ≤ 720）+ JPEG 质量 0.7
> - 或 Native 侧写入 app 私有文件，只回传 `file://` 路径由 JS fetch
> - 保持现有 ~4s 间隔（VLM 视觉链路不需要更高频）

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
| 3 | 截屏回传 | `capture.request()` → 壳回 `capture.frame`（软件层 draw 截 WebView 自身） | M3-part1 ✅ 壳侧已实现；全屏 MediaProjection 感知 M3-part2 |
| 4 | wasm 渲染 | Android WebView 上 ≥ 45fps（桌面实测 120fps） | 待真机 |
| 5 | 素材加载 | `asset.fetch` 拿到角色素材，商业版能显示角色 | 桥已通，Rust 侧对接 M3 |
| 6 | 音频 | 桌宠语音能播（Android WebView 音频策略需验证） | 待真机 |
| 7 | 体积/启动 | APK 体积与启动时间与现有原生版对比（需给出数据） | 体积✅ / **启动 2.1s✅** |

**第 7 项必须有数据** —— 这是决定要不要切换的关键，不能只凭感觉。

### 6.0 已跑通的端到端环节（AVD，2026-09-21）

在 `mc_test`（Android 16 / x86_64）上**人工跑通到这一步**：

| 环节 | 结果 |
|---|---|
| 出包 | ✅ `dist-shell/cute-pet-shell.apk` 3.5 MB |
| `adb push` + `pm install -r -t` | ✅ 安装成功，`pm list packages` 可见 `rust.cute_pet` |
| 启动 `MainActivity` | ✅ 起得来，首帧绘制 **2125 ms**（`finishDrawing of relaunch`）—— **这是启动时间的第一个实测值** |
| `OverlayService` 悬浮窗 | ⏳ 未跑到（见下方限制） |

安装与启动这两步走通说明**壳本身没结构性错误**：签名有效、Manifest 正常解析、
包名正确、方法数与 dex 没问题、framework API 调用不炸。

### 6.2 环境限制：本机 AVD 撑不到悬浮窗验证

在这台机器上反复出现：**模拟器每次都在 `Boot completed` 之后约 1 分钟内被杀掉**。
已验证与启动参数无关（试过有窗口 / `-no-window` / `-no-audio` / swiftshader），
日志每次都停在 `INFO | Boot completed in ... ms` 之后，没有异常栈。
判断是宿主的后台进程回收策略在杀高内存进程（同一环境里 `adb wait-for-device`
这类长时间阻塞的命令也会被 auto-background 后终止）。

结论：**悬浮窗的透明/帧率/拖动手感必须在真机上验**，模拟器救不了。
真机验证步骤见 `pet/webview/android/README.md`；CI 产物可从
`Android WebView Shell` workflow 的 artifact 直接下载。

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
- **M3 截屏 + 素材**
  - 截屏 `capture.request`：**M3-part1 ✅ 已实现**（软件层 draw 截 WebView 自身画面 → `capture.frame` base64）。
    全屏 MediaProjection 感知（喂 VLM）仍是 **M3-part2 待做**，复用 `pet/java/ScreenCaptureService.java`。
  - `asset.fetch`：**壳侧已通**（PetBridge 读外部素材目录）；**Rust/wasm 侧对接未做**——
    唯一触碰跨平台游戏核心的同步→异步改造，风险高，留作独立 spike（见 §9）。
  - `targetSdkVersion` 33 → **34** ✅（specialUse 前台服务类型，否则 Android 14 直接崩）
- **M5 丛雨当 agent 的脸（MVP）** ✅ Bridge 协议扩展（`agent.approve` + `agentSay`/`agentPropose`）
  + 内置 mock agent 端到端可演示（浏览器 `file://` / 真机壳均跑通）；
  真大脑（lilyco-approve 的 `router_v13` + `AgentOps`）对接契约见 §12.4，待并入原生层 `pet/java`
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

## 11. asset.fetch 的 Rust/wasm 侧对接（M3-part2 前的决策记录）

**问题**：`load_asset(p)` 在 wasm 上 `external_asset_dirs()` 返回空 → 角色素材走
`character_asset_missing_msg`（告诉用户 WASM 无本地文件）。壳已能通过 `PetShell.fetchAsset(path)`
（JS 桥→`PetBridge.readAssetFile` 读设备外部目录）拿到素材，但 wasm 内核**还不会改走这条桥**。

**难点（为何延后）**：Rust 的 `load_asset` 是**同步**返回 `Vec<u8>`，而桥是 **async(Promise)**。
调用点散布（chat.rs、run.rs 的图层/字体/语音/vlm_config 等），且 `load_asset` 是**跨平台共享核心**，
改动必须 `#[cfg(target_arch="wasm32")]` 隔离，不能影响桌面/原生 Android/鸿蒙/网页版。

**推荐方案（待 spike 验证）**：在 wasm 上引入"虚拟资产 FS"——
1. 桥新增 `asset.list`（枚举外部目录），或壳在启动时把约定素材清单推给 JS；
2. 内核启动早期（渲染前）**异步预取**用户目录全部素材，灌入一个 `static HashMap<String, Vec<u8>>`；
3. `load_asset` 在 wasm 下先查这张表，命中即返回，未命中再走原 `CoreAsset::get`/报错。

**不做的替代**：让每处 `load_asset` 调用点变 async 重构量太大、易错，否决。

**前提假设（需用户确认）**：桌宠角色素材体量小（图集/语音/字体），全量预取内存可接受；
若素材很大，需要改走"按需 fetch + 缓存"的异步加载路径（改动更大）。

**交付顺序**：先做本 §11 的极小 spike（仅 `asset.list` + 一处 `load_asset` 走虚拟 FS 验证可行性），
再决定是否全量改造。

---

## 12. 丛雨当 agent 的脸（MVP，2026-09-21）

把 WebView 壳扩成「agent 的脸」：**丛雨（Web 内容层）负责说台词 + 弹审批卡 + 收批准**；
真大脑（lilyco-approve 的 `router_v13` + `AgentOps` 无障碍执行）**暂不合并进此壳**，
留对接契约（§12.4）。WebView 壳跑不了 llama.cpp、碰不到无障碍服务，大脑必须留原生层。

### 12.1 Bridge 协议扩展（agent 脸）

新增：

**JS → Native**（用户点审批卡后，`PetBridge.postMessage`）
| 方法 | 用途 |
|---|---|
| `agent.approve { approved:bool, actions:[...] }` | 丛雨弹审批卡，用户点「批准/拒绝」后回传；`actions` 为原样动作对象数组 |

**Native → JS**（真大脑驱动丛雨，经 `evalJs` 调 `window.PetShell.*`）
| 函数 | 用途 |
|---|---|
| `window.PetShell.agentSay(text)` | 丛雨说一句台词（顶部气泡） |
| `window.PetShell.agentPropose(actions[])` | 丛雨弹审批卡（底部深色卡，列出动作 + 批准/拒绝按钮） |
| `window.PetShell.agentApprove(approved, actions)` | JS 内部用：按钮点击 → 回传 native / 浏览器本地模拟 |

Java 侧新增 `PetBridge.Host` 方法 `agentSay(String)` / `agentPropose(JSONArray)` / `onAgentApproved(boolean, JSONArray)`；
`OverlayService` 实现它们，并在 `onPageFinished` 启动内置 mock agent（演示大脑）。

### 12.2 动作 schema（对齐 lilyco-approve 的 AgentAction）

审批卡里每个动作对象，字段对齐 `lilyco-approve` 的 `AgentAction` sealed interface：

| type | 字段 | 中文描述（`describeAction`） |
|---|---|---|
| `tap_text` | `text` | 点击文字："xxx" |
| `tap` | `x`, `y` | 点击坐标 (x, y) |
| `input` | `text` | 输入文字："xxx" |
| `fill_text` | `label`, `text` | 填写 label = text |
| `open_app` | `pkg`, `label` | 打开应用：label |
| `back` | — | 返回 |
| `scroll_down` | — | 向下滚动 |

> `describeAction` 在 JS（`pet_bridge.js`）与 Java（`OverlayService.describeAction`）两端各实现一份，
> 保证审批卡展示与 logcat 日志描述一致。

### 12.3 最小可演示路径

- **浏览器（无原生桥）**：直接 `file://` 打开 `pet/webview/web/index.html`，
  内置 JS mock agent 自动跑：丛雨说一句 → 1.3s 后弹审批卡（open_app 微信 + tap_text + tap）→
  点「批准」本地模拟执行并说「好嘞～我这就去办！」。
- **真机壳**：`OverlayService.onPageFinished` 用 `evalJs` 启动同一个 mock agent，
  用户点「批准」→ `agent.approve` 回 native → `onAgentApproved` 打印 `describe` 日志
  （MVP 仅模拟，不真执行无障碍）。

### 12.4 接 lilyco-approve 真大脑的对接契约

后续把真大脑并进 cute-pet 原生安卓层（`pet/java`）时：

1. 真大脑产出「台词 + 动作列表」，调用 `agentSay` / `agentPropose` 驱动丛雨（复用本壳 Bridge）。
2. 用户点「批准」→ `onAgentApproved(true, actions)` 里**改调 `AgentOps.runA11y(actions, ctx)`**
   真正执行无障碍动作（替换 MVP 的本地模拟 log）。
3. `OverlayService` 里 `onPageFinished` 启动 mock agent 的那一行**删除**，改由真大脑生命周期驱动。
4. 动作 schema 已与 `AgentAction` 对齐，无需再转换；若 lilyco-approve 新增动作类型，
   需在 JS/Java 两处 `describeAction` 同步补描述。

---

## 13. 语音输出（TTS，2026-09-22）

让壳「不只弹字，还能出声」。**零新增原生依赖**。

**接口契约**（与原生版 `PET_TTS_URL` 完全同款，服务端可互换）：
```
GET {base}/tts?text=<urlencoded>   ->   audio/wav
```

**Web 侧（`pet/webview/web/pet_bridge.js`）**
- 新增 `window.cutePetTTS = { enabled, base, setBase(url), speak(text), stop() }`
- `PetShell.agentSay(text)` 现在**同时驱动气泡 + 语音**（原来只渲染气泡）；
  审批通过/拒绝后的本地回话也改走 `agentSay` → 一起出声。
- 地址来源优先级：`?tts=<url>` 查询参数 > `localStorage['pet_tts_url']`。
- **未配置 = 静默禁用**：不发任何请求、不报错。失败**只提示一次** `console.warn`
  （遵循项目「高频 warn 会爆 console」的教训，见 §4）。
- 播放用 `new Audio(URL.createObjectURL(blob))`；自动播放被拦时静默忽略。

**原生侧（`OverlayService.java`）**
- `PREF_TTS_URL = "tts_url"`（SharedPreferences 名 `pet`）→ `buildContentUrl()` 拼 `?tts=`。
- 未配置时 `buildContentUrl()` 返回原 `CONTENT_URL`，**行为与以前完全一致**。
- 设置方式：`adb shell` 写 SharedPreferences（或代码预置），无需重编译。

**⚠️ 必须开明文流量**：`AndroidManifest.xml` 加 `android:usesCleartextTraffic="true"`。
Android 9+ 默认禁明文，不开则 `http://` TTS **静默失败**（最难查的那种）。
影响面受控：壳**不进 Release**（artifact-only），且语音**默认关闭**（未配 tts_url 零请求）。

**本地 TTS 服务**：`tts-spike/tts_server.py`（ZipVoice int8 distill zh-en + vocos 声码器，CPU 实时；
接口同上）。也可换成任意实现同契约的服务（含云端）。

**验证**：复刻 `pet_bridge.js` 的 URL 构造打本地服务 → `200 / audio/wav / RIFF / 3.54s` ✅。
**待办**：真机验证（需把 `tts_url` 指向真机可达的服务，例如局域网内 PC 的 `http://192.168.x.x:7860`）。

### 13.1 壳内合成（WebView 内跑 TTS，手机单机可用）

优先级：**① 壳内合成 → ② HTTP 服务**（`cutePetTTSLocal` 存在就先用它，失败/不可用再回落 `?tts=`）。

**引擎**：`kokoro-js-zh`（`chalecao/kokoro-multilang-zh`）+ `Kokoro-82M-v1.0-ONNX`。
- 模型 **Apache-2.0**，自带中文音色：`zf_xiaobei/zf_xiaoni/zf_xiaoxiao/zf_xiaoyi`（女）、
  `zm_yunjian/zm_yunxi/zm_yunxia/zm_yunyang`（男）。
- 🔴 **音色是 Kokoro 自带中文女声，不是丛雨**；要丛雨音色仍走 ②（ZipVoice 服务）。

**随壳分发（`web/vendor/kokoro-zh/`，约 19MB，已进 build.sh）**：
- `kokoro.web.js`（901KB，**自带 transformers.js，无外部 import**）
- `espeak-ng.wasm`（18MB，**中文 G2P**；⚠️ 必须与 `kokoro.web.js` **同目录** ——
  代码用 `new URL("espeak-ng.wasm", import.meta.url)` 定位）
- ⚠️ 这两份**必须自托管**：npm 包 `kokoro-js-zh` 的发布**漏了** `espeak-ng.wasm` 和 `voices/`，
  直接用 CDN 会 404。`kokoro.web.js` 取自包内，wasm 取自仓库
  `chalecao/kokoro-multilang-zh/raw/main/dist/espeak-ng.wasm`。

**首次运行**：模型(~82MB q8)/音色(~510KB) 从 HF 拉取并由浏览器缓存（Cache API）→ **之后可离线**。
onnxruntime-web 的 wasm 默认走 jsDelivr CDN（如需全离线，设 `env.backends.onnx.wasm.wasmPaths` 指向本地）。

**Web 侧 API**：`window.cutePetTTSLocal = { model, voice, ready(), load(), speak(text), stop() }`
（`web/pet_tts_local.js`；`speak` 返回 `true`=已播放、`false`=未播放交给上层兜底）。

**状态**：已接线 + `node --check` 通过 + vendored 与源包 **md5 逐字节一致**；
⚠️ **浏览器/WebView 运行时未验**（本机无浏览器自动化），且 small ASR 对合成音色回读不可靠
→ **需真机/浏览器实听确认可懂度**。

#### 13.1.1 实测踩到的两个坑（必须照做，否则静默坏掉）

1. **音色 `.bin` 必须放在壳内 `web/voices/`**。
   加载器是 `fetch('./voices/<名>.bin').catch(() => fetch(HF))` —— **先取页面相对路径**，
   只有 **fetch 抛异常**才回落 HF。而静态服务器/WebView 对缺失文件返回 **404 + HTML 正文**
   （fetch **不抛错**）→ HTML 被当成浮点数据 → `RangeError: byte length of Float32Array should be a multiple of 4`。
   → 已随壳打 8 个中文音色（`web/voices/`，4.1MB）；**注意清 `kokoro-voices` 缓存**（坏响应会被缓存）。
2. **必须 `device:'wasm'`，不要用 WebGPU**。
   实测 WebGPU EP 输出坏音频（`max_volume 0.0dB` 削顶 + `mean -32dB` 近静音，ASR 只出幻觉）；
   wasm 正常（`max -4.3dB / mean -20.8dB`）。`pet_tts_local.js` 的 `pickDevice()` 已恒返回 `'wasm'`。
   代价：wasm 较慢（实测 RTF≈1.8）。

**已验证（Playwright + 真浏览器）**：模块加载 → 模型加载（voices=39, zh=8）→ 合成出 6.75s 音频。
**质量现状**：ASR 显示「**后半句逐字全对、前半句糊**」（如「…今天过得开心吗？」完全正确）。
→ **技术通路成立，但质量未达产品级**；提升需移植官方 `misaki[zh]` 的中文 G2P。

#### 13.1.2 🔴 结论：本方案**不可用于产品**（根因＝音素集不对）

用户实听确认「前面听不清」。对照实验定论：

| 来源 | 「你好，我是你的桌宠。」的音素 |
|---|---|
| **官方 `misaki[zh]`** | `ni↓xau↓, wo↓ ʂɨ↘ ni↓ tɤ ꭧwo→ꭧʰʊ↓ŋ.`（声调箭头 + IPA） |
| 本包的 espeak(cmn) | `n_i214_X_'Au214__\| j_'iA11__\| …`（下划线＋数字声调） |

**两套符号集完全不同**，而模型 tokenizer（178 词）是按 misaki 那套训练的 → 前糊后准。

**官方 Python 管线对照（`kokoro` + `misaki[zh]`，`repo_id=hexgrad/Kokoro-82M-v1.1-zh`）逐字全对**，
证明**模型没问题，问题 100% 在社区的 espeak 音素化路线**。

**⇒ 正确方向：`sherpa-onnx`**（C++；有 **Rust 绑定 `sherpa-rs`** 与**官方 Android SDK/AAR**）。
它自带 `lexicon.txt` 做 G2P —— 我们早前用它跑 ZipVoice 时**中文 ASR 逐字全对**。
壳虽是 WebView，但可从 **Java 层调 sherpa-onnx Android SDK** 实现原生端侧中文。
本节 13.1 的浏览器方案保留作技术存档，**默认不要用于生产**。

---

## 14. 原生离线 TTS：sherpa-onnx 落地（2026-09-23）

按 13.1.2 的结论落地：**Java 层集成 sherpa-onnx AAR，模型随 APK 资产分发，完全单机离线**。

### 14.1 架构

```
WebView(pet_bridge.js ttsSpeak)
  ① HTTP 服务(?tts=/PET_TTS_URL, 丛雨 ZipVoice 音色) —— 配置了才走, 失败自动回退
  ② 壳内原生引擎 TtsEngine(sherpa-onnx VITS)  ←←← 新增, 单机兜底, 本节主角
  ③ WebView 内合成(pet_tts_local.js)           —— 归档, 默认关闭(见 13.1.2)
```

- `TtsEngine.java`：包一层 `com.k2fsa.sherpa.onnx.OfflineTts`。
  **模型直接从 APK assets 读**（`new OfflineTts(getAssets(), config)`），不拷 filesDir。
  单 worker 线程串行「合成 → AudioTrack 播放」；`speak()` 新请求顶掉旧 pending 并
  打断正在播的音频（`playGen` 代次计数）；引擎 init(~1-3s) 期间到来的 speak 排队等。
- `PetBridge`：协议加 `tts.speak {text}` / `tts.stop`；`shellInfo` 加 `nativeTts:true` 能力位。
- `OverlayService`：onCreate 建 `TtsEngine`（后台 init），onDestroy release；
  说话人 id 走 SharedPreferences 键 `tts_native_sid`（默认 0，aishell3 共 174 人）。

### 14.2 模型与构建（build.sh 3.5 节）

| 项 | 值 |
|---|---|
| 运行时 | `sherpa-onnx-1.13.8.aar`（47MB；classes.jar 进 dex，arm64-v8a 4 个 .so 进 `lib/`） |
| 模型 | `vits-icefall-zh-aishell3`（aishell3 多说话人中文，30MB） |
| 进包文件 | 仅 `model.onnx / lexicon.txt / tokens.txt / date.fst / number.fst` |
| **rule.far** | **180MB 的 jieba 大词典，绝不进包** —— 数字/日期读法用 `date.fst+number.fst` 已够（官方 README 同款） |
| kotlin-stdlib | **必须一起 dex**（2.0.21，1.7MB）：sherpa 的 Java 层是 Kotlin 编译的，缺 stdlib = 运行时 `NoClassDefFoundError: kotlin.jvm.internal.Intrinsics`，**编译期无任何报错** |

第三方输入全在 `pet/webview/android/third_party/cache/`（gitignored），build.sh 缺失时拉取。
APK 体积：~25MB → **51MB**。

### 14.3 顺手修掉的旧坑

- build.sh 组装 assets 前不清理 → `cp -r vendor` 对已存在目标拷成 `vendor/vendor/`
  嵌套，每次构建把 19MB 死重复制进包（本次 59MB→51MB 即由此而来）。现组装前 `rm -rf`。
- voices/*.bin 的 fetch-if-missing 段是死代码（拉了从不进包），删除。

### 14.4 验证状态与下一步

- ✅ 本地 `build.sh --out dist-shell` 全链路通过（javac/d8/打包/签名）；
  APK 结构核验：`lib/arm64-v8a/` 4 个 .so、`pet/tts/` 模型 5 件、dex 含 sherpa + kotlin 类。
- ⏳ **真机出声验证待做**（AVD 撑不到悬浮窗，见 §6.2）：
  `adb install -r dist-shell/cute-pet-shell.apk` → 启动悬浮窗 → 点桌宠触发 mock agent 台词 →
  `adb logcat -s CutePetTts` 看 `init ok` / `generate` 耗时。
- 第二步（已预留）：换 **ZipVoice int8 + 丛雨参考音** —— 1.13.8 已内置
  `OfflineTtsZipVoiceModelConfig`；参考音走用户自备素材目录（合规，不进包）。

---

## 10. 一句话总结

> 走 wasm + WebView 能砍掉的是「逻辑/渲染层的多平台编译与适配」，
> 砍不掉的是「悬浮窗 / 截屏 / 前台服务」这些系统能力 —— 后者继续用你已有的
> `OverlayService` / `ScreenCaptureService`，只是内容区从 `QuadSurface` 换成 `WebView`。
> Android 是收益最大的第一站（正好绕开 plyx / NDK / 沙箱路径这些坑），
> 但**必须用 Java 壳，webview-c 帮不上**；iOS 无解，不要投入。
