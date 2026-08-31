# 已验证问题解决手册 (Troubleshooting)

> 本项目在开发中解决过的**已验证**问题清单。每个问题含: 症状、根因、修复、已验证结果。
> 目的: 让 agent 遇到同类问题时**直接定位**, 不重复踩坑。
>
> 相关指南: [CloudStudio 认证](cloudstudio-access.md) · [GPT-SoVITS 丛雨合成](cloudstudio-tts-guide.md) · [Android 悬浮窗视觉](lyco-android-overlay-vision.md) · [cargo-quad-apk 工具链](toolchain-cargo-quad-apk-fork.md)

---

## 一、桌宠功能问题(已修复)

### 1. 输入框只能一个字符 / 多字符输入被反转
- **症状**: Android 输入 "murasame" 显示为 "emasarum"(完全反转); 单字逐个输入。
- **根因**: `macroquad-ply` 的字符队列用 `Vec::pop()`(LIFO)取出。字符按序 push 进队列,
  但 `get_char_pressed()` 从队尾 pop → 逆序。多字符输入被反转。
- **修复**: `pet/vendor/macroquad-ply/src/input.rs` — 改 FIFO(队首 `remove(0)`),
  `get_char_pressed()` / `get_char_pressed_ui()` 两处。并 vendored `macroquad-ply` 到
  `pet/vendor/` + Cargo.toml `[patch.crates-io]` 保证持久。
- **验证**: 模拟器输入 "konbanwa" 按序显示 ✓(v0.2.2)
- **注意**: 此 bug 影响所有平台(桌面/WASM/Android), 修复全局生效。

### 2. 语音切换后只有中文
- **症状**: 切到日语模式, TTS 失败/超时仍播中文问候语音+中文气泡。
- **根因**: `pet/src/main.rs` 两条 TTS 兜底路径硬编码 `cn_voices`, 不分 `lang`。
- **修复**: `main.rs:904` / `main.rs:923` — 兜底按 `lang` 选库(日语→`jp_voices` mur001 原声)。
- **验证**: 编译通过; 逻辑按语言分派。

### 3. 日语模式用中文回应
- **症状**: 设置日语, 输入后丛雨用中文回复。
- **根因** (多成因):
  1. 预置问答 `preset_match` 不分语言 → 日语模式命中中文问答对(dialog_preset.txt 是中文)。
  2. `llm_hint` 初始化顺序: 先 `apply_ui_lang`(设为日语)后被硬编码中文覆盖。
  3. L 键切换提示恒为中文; `(无回复)` 恒中文。
- **修复**:
  - `main.rs:1216` — `preset_hit = if lang == Lang::Zh { ... } else { None }`(日语跳过中文预置)。
  - `main.rs` 初始化 — 先设 `llm_hint` 再 `apply_ui_lang`(让它按语言挑文案)。
  - L 键切换 / `(無回复)` → 双语文案。
- **验证**: 审查 agent 全链路确认日语模式无中文泄漏。

---

## 二、构建链路问题(已修复)

### 4. Android 构建 javac 失败(143 错误)
- **症状**: `plyx apk --native` 在 javac 阶段报 143 个 "GBK 不可映射字符" 错误。
- **根因**: vendored `.java` 文件(MainActivity.java/QuadNative.java)含 UTF-8 中文注释,
  但 `cargo-quad-apk` fork 的 javac 未指定 `-encoding`, 默认工作区 GBK(中文 Windows)。
- **修复**: `cargo-quad-apk-ply` fork `src/ops/build.rs` — javac 命令加 `-encoding UTF-8`。
  (需重新 `cargo install` 该 fork 才生效)
- **验证**: 修复后 javac 只剩 deprecation 警告, APK 构建成功。

### 5. APK 构建时 `include_str!("../assets/...")` 失败
- **症状**: 构建 APK 报 `couldn't read src\..\assets\murasame_persona.txt`。
- **根因**: cargo-quad-apk 把源码复制到临时目录, `include_str!` 相对路径解析失败;
  且源码复制与并行构建竞态污染临时目录。
- **处理**: 清理临时构建目录 `plyx-apk-native` 后干净重试(`rm -rf` 该目录)可恢复。
  这是临时目录竞态, 非代码改动。

---

## 三、GPT-SoVITS 合成问题(已验证)

### 6. 日语合成乱码(核心难点)
- **症状**: 让模型"照读原声同一句"都对不上(large-v3 识别为完全不同的内容),
  而原声本身能被 large-v3 100% 识别。
- **根因**: GPT(语义模型)用了丛雨微调版 `murasame_s1.ckpt`(仅 300 条语料, 过拟合 →
  语义 token 预测崩)。音色 SoVITS 用丛雨微调是正确的。
- **修复**: GPT 改用**通用大模型** `GPT_weights_v2Pro/lain-e15.ckpt`(语义泛化好)
  + 丛雨 SoVITS(音色) → 合成内容正确。
- **验证**: 合成「主と一緒にいられて、吾輩は嬉しいぞ」, large-v3 识别为
  「ウェシと一緒にいられて、ワダハイは嬉しいぞ。」(内容几乎全对, 仅丛雨口音音素差异)。
- **细节**: 参考音频必须 3~10 秒; 参考文本=参考音频原台词; 用 `inference_cli.py`(非 webui)。
- **完整流程**: [docs/cloudstudio-tts-guide.md](cloudstudio-tts-guide.md)

---

## 四、环境备忘

| 问题 | 处理 |
|---|---|
| 模拟器无声 | 启动带 `-audio host`(默认 MUSIC 流衰减到 -33dB 近无声) |
| 模拟器音频会话断 | 重启模拟器恢复 |
| 字符输入反转 | 见 §1(macroquad-ply FIFO) |
| 中文克隆口音 | 丛雨模型纯日语, 中文合成口音重属正常, 非 bug |

---

## 五、进度记录

本次里程碑(2026-08-31):
- 修复 issue #1(输入逆序) / #2(语音切换) / #3(日语回显) → **v0.2.2 已发布**
- 定位并解决日语合成乱码根因(通用GPT+丛雨s2)
- 建立 CloudStudio GPT-SoVITS 合成 + 认证两条可复用指南

> 注: `progress.md` 有完整桌面宠开发历史; 本文件聚焦**已验证问题的快速定位**。
