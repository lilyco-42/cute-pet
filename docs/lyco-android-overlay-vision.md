# lyco 探索: 安卓悬浮窗 + 第三方接入 + 视觉模型（小丛雨看着你玩游戏）

> 调研日期: 2026-08-19 · 方法: lyco 预研先行（gh 搜索 + 论坛/web + 源码级克隆研究）
> 目标: 9 月 1 日前完成三项功能 + 最简功能测试

## 一行需求（已与用户确认）

**安卓端小丛雨以系统悬浮窗浮在全屏游戏上，接入第三方云 API（LLM/视觉/TTS），
用 MediaProjection 截屏 + 云端 VLM 理解游戏画面并做出反应。**

## 结论速览

| 功能 | 决策 | 一句话证据 |
|---|---|---|
| 悬浮窗 | **fork/extend（自研为主，借模式）** | 候选项目全用 WebView/SVG 重画桌宠，无法保留丛雨 Rust 渲染/语音/语料；但「FGS + WindowManager + TYPE_APPLICATION_OVERLAY」模式已被 AI-Live-Overflow 族验证，且 miniquad-ply 渲染表面就是普通 SurfaceView，可挂进悬浮窗 |
| 第三方接入 | **build（小，扩展现有链路）** | pet 已有 ureq + OpenAI 兼容 `respond_llm`，加视觉/换 TTS 只是扩展同一 HTTP 层 |
| 视觉模型 | **build（组合件：MediaProjection + VLM API）** | 候选均无实时屏幕理解（只监听截图文件夹/前台 app）；VLM 用 GLM-4V-Flash（免费）/ Qwen-VL（低价），API 与现有 LLM 同构 |

## 候选项目（gh 实测，均有 URL）

### 安卓悬浮桌宠（悬浮窗先例）
| 仓库 | star | license | 最近 push | 价值 |
|---|---|---|---|---|
| [Vael-KY/AI-Live-Overflow](https://github.com/Vael-KY/AI-Live-Overflow) | 0 | CC BY-NC-SA 4.0（非商用） | 2026-08 | **架构蓝图**：FGS+WindowManager+透明 WebView+SVG、手势、前台感知、截图感知、保活。最佳模式参考 |
| [zayne1006/AI-Live-Pet](https://github.com/zayne1006/AI-Live-Pet) | 0 | 无 | 2026-08-02 | Compose 实现 + 情绪/传感器/手势/皮肤管理 + GitHub Actions 自动出 APK。**具体实现参考** |
| [muxiaoqi007/aqua-pet](https://github.com/muxiaoqi007/aqua-pet) | 0 | 无 | 2026-08-10 | Kotlin 精灵动画悬浮 + 养成系统（Roam overlay） |
| [huise513/floating-pet](https://github.com/huise513/floating-pet) | 0 | 无 | 2026-04 | C++ 悬浮宠物（QQ 宠物风），NDK 参考 |
| [Sora-Shiro/LemonYi](https://github.com/Sora-Shiro/LemonYi) | 7 | GPLv3 | 2026-02 | Android 桌宠（Java），GPL 传染需注意 |
| [w006gy/shimeji](https://github.com/w006gy/shimeji) | 0 | 无 | 2025-02 | shimeji 桌宠移植参考 |

> 注意：这些项目大多**无 license 或 CC BY-NC-SA / GPL** —— 只可研究借鉴，不可直接复制进发布版。
> 衍生族（chi-overflow/StarPet/black-panther-pet/konata-deskpet 等）均 fork 自 AI-Live-Overflow。

### 屏幕理解 / GUI Agent（视觉方向积木，v1 不直接用）
| 仓库 | star | license | 用途 |
|---|---|---|---|
| [microsoft/OmniParser](https://github.com/microsoft/OmniParser) | 25.3k | CC-BY-4.0 | 屏幕解析为结构化 UI 元素（GUI agent 专用，v1 过重） |
| [X-PLUG/MobileAgent](https://github.com/X-PLUG/MobileAgent) | 9.1k | MIT | 手机 GUI agent（自动操作手机，非「看着+吐槽」） |
| [kevinhsp/screen-understanding-agent](https://github.com/kevinhsp/screen-understanding-agent) | 1 | MIT | 屏幕理解 agent（Python，思路可参考） |

### 现成视觉 API 桥（免费/低价）
- [gmleong/dsh-img](https://github.com/gmleong/dsh-img)：文本模型接视觉 API 的插件，默认 **GLM-4V-Flash（免费，智谱 open.bigmodel.cn）**，备选 Qwen-VL（阿里免费额度）/ Ollama 本地 `minicpm-v:8b` / 任意 OpenAI 兼容端点；多后端 failover + 结果缓存。**与 pet 现有 LLM 调用完全同构**。

## Niche 知识（文档外踩坑点，均有出处）

### 悬浮窗
- 悬浮窗 = 前台服务(FGS) + `WindowManager.addView` + `TYPE_APPLICATION_OVERLAY`(API 26+)，配 `SYSTEM_ALERT_WINDOW`；`FLAG_NOT_FOCUSABLE|FLAG_NOT_TOUCH_MODAL|FLAG_LAYOUT_IN_SCREEN|FLAG_LAYOUT_NO_LIMITS` 保证不抢焦点、可拖出屏、沉浸式适配（[juejin 全屏悬浮窗适配](https://juejin.cn/post/7219109751367417917)、[AI-Live-Overflow overlay-service.md](https://github.com/Vael-KY/AI-Live-Overflow/blob/main/docs/overlay-service.md)）。
- 国产 ROM（小米/华为/魅族等）悬浮窗授权入口各不相同，需按机型跳对应设置页 —— 现成方案 [FloatingPermissionCompat](https://wenku.csdn.net/doc/4u59npg12o)（适配小米/华为/魅族/OPPO 运行时授权）；部分 ROM 游戏空间/性能模式会拦截悬浮窗，需引导用户加白名单/关闭游戏模式拦截。
- Android 15（targetSdk 35+）起悬浮窗不允许盖在敏感界面（锁屏/通话/系统设置等）上，**游戏不受限**（[Android 15 behavior](https://developer.android.com/about/versions/15/behavior-changes-15)）。

### MediaProjection（视觉截屏，版本约束随年份收紧）
- **Android 14+**：录屏必须走 `foregroundServiceType="mediaProjection"` 的前台服务 + manifest 声明 `FOREGROUND_SERVICE` / `FOREGROUND_SERVICE_MEDIA_PROJECTION`；每次捕获会话都要用户授权，**停止后重建必须重新授权**（[官方 MediaProjection 文档](https://developer.android.com/media/grow/media-projection)）。
- **Android 14** 引入「单应用屏幕共享」（只截某个 app 窗口）—— 可只截游戏、把自家悬浮窗排除在画面外。
- **Android 16（API 36）**：锁屏（电源键/自然锁屏）自动停止 MediaProjection，新增 MediaProjectionStopController；必须注册 `MediaProjection.Callback.onStop()` 清理资源并引导重新授权（[Android 16 适配](https://lastwarmth.win/2026/07/29/Android16/)）。
- 设计含义：**「看着玩游戏」= 定时/触发式截帧 → JPEG → VLM，不是持续视频流**（省流量省成本、绕开会话限制）；锁屏/切走自动停，回来自动重连。

### 视觉模型 API（中国区，成本≈0）
- GLM-4V-Flash：**免费**多模态（[智谱官方](https://mp.weixin.qq.com/s/YD0yeueTgoCa8Gy-erpeHw)），OpenAI 兼容 `/v1/chat/completions`。
- Qwen-VL（qwen-vl-plus/max）：阿里百炼，OpenAI 兼容模式，图片输入按 token 计费、价格低（[qwen-vl-plus](https://help.aliyun.com/zh/model-studio/qwen-vl-plus)）；有免费额度。
- 备选：Ollama 本地 `minicpm-v:8b`（离线，v1 不要求）、任意 OpenAI 兼容网关（GPT-4o 等）。
- 延迟 1-3s/图，适合事件驱动（点「看看我在干嘛」/ 每 10-30s 一次 / 画面变化触发），不适合逐帧。

## 目标架构（要点）

```
Android APK (cargo-quad-apk 构建, 额外 Java 文件机制)
├─ MainActivity  → 权限引导(SYSTEM_ALERT_WINDOW/通知/录屏授权) + 设置
├─ OverlayService (FGS, 常驻通知)
│    └─ WindowManager.addView(QuadSurface, TYPE_APPLICATION_OVERLAY)
│         └─ SurfaceView = miniquad-ply 现成渲染表面(Rust 丛雨原样渲染)
├─ MediaProjectionService (FGS type=mediaProjection)
│    └─ ImageReader 截帧 → JPEG → ureq POST VLM(OpenAI 兼容)
└─ Rust 侧(现有 pet): respond_llm 扩展 image content → 反应台词 → 语音/表情
```

- **悬浮窗关键可行性**（已验证源码）：`pet/vendor/miniquad-ply/java/MainActivity.java` 中渲染表面是普通 `SurfaceView`（`QuadSurface`），surface 经 JNI 交给 Rust 建 EGL 上下文 —— SurfaceView 可被 `WindowManager` 加到任意悬浮窗窗口；miniquad-ply 是 vendored 版，可改。cargo-quad-apk 支持额外 Java 文件（`[package.metadata.android] java_packages / java_files`）随 APK 编译，权限直接在现有 `[package.metadata.android.permission]` 加。
- **风险点（需最小化验证）**：① 悬浮窗模式下 GL 渲染线程由 Service 驱动而非 Activity（需把 surface 生命周期回调从 Activity 移到 OverlayService 的 QuadSurface）；② 悬浮窗里聊天输入法弹出（overlay 暂不设焦点，聊天走全屏 Activity 或气泡快捷回复）；③ 自家悬浮窗会出现在 MediaProjection 截屏里（可用单应用共享排除，或接受）。

## 9/1 路线图（最小验证先行，原子化构建）

| 阶段 | 内容 | 最简验证 |
|---|---|---|
| W1 (8/19-8/25) 悬浮窗 spike | 改 vendored MainActivity 增加 overlay 模式；新增 OverlayService.java + 权限清单；QuadSurface 挂进 TYPE_APPLICATION_OVERLAY；手势拖动/点击说话 | APK 装真机：悬浮窗盖住全屏游戏、可拖动、点击有语音 |
| W2 (8/25-8/29) 视觉链路 | MediaProjectionService 截帧→JPEG；respond_llm 支持 image content（GLM-4V-Flash 免费 key）；触发机制（按钮/定时/画面变化） | 点「看看我在干嘛」→ 小丛雨说出游戏画面相关台词 |
| W3 (8/29-9/1) 收尾 | 权限引导（悬浮窗/录屏/通知/电池白名单，参考 FloatingPermissionCompat 思路）；悬浮窗模式聊天入口；真机回归（含锁屏重连）；发版 | 三功能端到端通过 + 发布 APK |

## 引用
- 候选仓库: 见上文表格 URL（gh search/gh repo view 实测）
- 悬浮窗: juejin 全屏悬浮窗适配 / FloatingPermissionCompat / AI-Live-Overflow docs
- MediaProjection: 官方文档 / Android 16 适配指南
- VLM: 智谱 GLM-4V-Flash 官方 / 阿里 qwen-vl-plus / dsh-img
- 本地源码: pet/vendor/miniquad-ply/java/MainActivity.java、pet/src/chat.rs respond_llm、cargo-quad-apk src/config.rs（额外 Java 文件机制）
