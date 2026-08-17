# cute-pet — 丛雨(ムラサメ)桌宠

跨平台桌宠雏形。基于 **ply-engine / macroquad** 渲染，从《千恋万花》步兵版 APK 提取真实立绘/语音素材。

## 现状
- ✅ 透明 + 置顶 + 无边框 + 自动裁剪窗口（**跨平台平台层**：Windows `windows.rs` + macOS `macos.rs` + Linux `linux.rs`，均已通过对应 target 交叉编译检查；运行验证需真机）
- ✅ 丛雨立绘分层合成（服装 × 表情 × 头发 × 腮红，manifest 驱动，z 序正确）
- ✅ **眨眼/口型动画**：待机每 2.5s 自动眨眼一次，说话/发声时按 8Hz 交替口型（b/e/m 合成表情层像素分析生成的孪生映射表驱动）
- ✅ 交互：点击说话（轮播丛雨语音 + 切表情）、拖拽移动、数字键切表情、D 切服装、Space 切姿势
- ✅ **聊天 UI**：Enter 输入 → 丛雨回复（LLM 或语料）+ 气泡显示
- ✅ **克隆音色 TTS**：聊天回复自动用远程 GPT-SoVITS 合成丛雨克隆音色播放（`PET_TTS_URL` 可改）
- ✅ **语气学习**：导入聊天记录（`PET_STYLE_LOG`）提取「喜欢的人」的说话风格注入 persona
- ✅ GPT-SoVITS 训练管线全通（s1 200ep + s2 100ep，T4 训练，推理服务常驻）

## 平台层
| 平台 | 文件 | 实现 | 状态 |
|---|---|---|---|
| Windows | `src/windows.rs` | WS_POPUP + WS_EX_LAYERED + DWM 逐像素透明 + 置顶 | ✅ 已验证 |
| macOS | `src/macos.rs` | AppKit NSWindow: setOpaque:NO + 透明背景 + 去标题 + 浮动层级 | ✅ 编译通过，待真机 |
| Linux | `src/linux.rs` | X11: _NET_WM_STATE_ABOVE 置顶 + _MOTIF_WM_HINTS 无边框 + 整窗透明度 | ✅ 编译通过，待真机 |

> Linux 逐像素透明需 ARGB visual + 合成器（当前为整窗 alpha）；Wayland 需 layer-shell，另案处理。

## 运行
```bash
cd pet
cargo run                 # 启动桌宠
PET_VERIFY=1 cargo run    # 2 秒后截图退出(验证用)
PET_DEBUG=1 cargo run     # 显示调试信息
cargo run --bin chat_demo -- "你好"   # 聊天 CLI demo
```

## 操作
| 键 | 功能 |
|---|---|
| Enter | 开始/提交聊天输入 |
| 左键点击 | 说台词 + 切表情（拖拽移动窗口） |
| 1-9 | 切表情（01/03/04/13/14/19/21/02/20） |
| D | 服装 私服↔洋装 |
| Space | 姿势 diff 1↔2 |
| E | 快速说一句台词 |
| F2 | 截图 |
| Esc | 退出 / 取消输入 |

## 聊天（AI 语气层）
`chat::Persona::respond_llm` 走 OpenAI 兼容接口，环境变量门控：
```
PET_LLM_BASE_URL=…   # 默认 https://api.deepseek.com
PET_LLM_API_KEY=…
PET_LLM_MODEL=…      # 默认 deepseek-chat
```
未配置密钥 → 自动回退 `respond_corpus`：从语料里按输入哈希取一条 + 对应语音引用。

**本地轻量 AI（自研 lyco_chat，跑在 CloudStudio 云 GPU）**：`lyco_chat`（`../lyco_chat`）提供 OpenAI 兼容服务，部署到云端后桌宠指向预览地址即可完全离线聊天（本机不跑重负载）：
```
PET_LLM_BASE_URL=https://<spaceKey>--8080.ap-shanghai2.cloudstudio.club
PET_LLM_API_KEY=cloudstudio
PET_LLM_MODEL=lyco
```
一键部署见 `../docs/lyco-chat-cloud-deploy.md`（认证 → 传源码 → 云端 cargo build → setsid 启动 serve → 预览地址）。模型训练也在云端 T4 GPU 完成。

**官方中译语料**：默认加载 `murasame_corpus_zh.jsonl`（4404 条，从汉化版 `patch.xp3` 剧本提取的丛雨官方中文台词，语音码与日文原版对齐）。设 `PET_CORPUS_JP=1` 可回退日文原版 `murasame_corpus.jsonl`。额外配置 `PET_LLM_API_KEY` 时，未匹配到中文的日文台词会自动 LLM 翻译（`chat::translate_to_chinese` + `has_kana`）。

### 学习「喜欢的人」的语气
导入 Ta 的聊天记录，桌宠回复就带上 Ta 的风格。两种数据源：

**A. 本地文件**（JSON/JSONL/纯文本/时间戳文本/HTML 导出/chatlog 工具 JSON）：
```bash
PET_STYLE_LOG=/path/chat.txt PET_STYLE_SPEAKER=名字 cargo run
```

**B. chatlog 工具 HTTP API**（`lilyco-42/chatlog` / `sjzar/chatlog`，微信数据库解密服务）：
```bash
# 先跑起 chatlog: chatlog 启动 → 解密数据 → 开启 HTTP 服务(默认 5030)
PET_CHATLOG_URL=http://127.0.0.1:5030 PET_STYLE_SPEAKER=名字 cargo run
```
桌宠直接调 `GET /api/v1/chatlog?talker=<名字>&format=json` 拉取记录并学习，无需中间文件。

`chatlog::extract_style` 提取平均长度/短句比例/emoji/口头禅/代表句 → 注入 LLM persona 系统提示。

聊天记录格式自动识别（`chatlog.rs`）：
| 格式 | 来源 |
|---|---|
| JSON `{"messages":[...]}` / 数组 | 通用备份 / chatlog 工具 API（`talker`/`talkerName`/`sender`/`content`） |
| 时间戳文本 `2023-08-14 18:26:35 名字` + 内容行 | 微信/QQ 电脑版导出 txt |
| HTML `<span class="name">` / `<b>名字</b>` | 微信/QQ 网页版导出 |
| 纯文本 `名字: 内容` | 通用 |

## 架构
```
src/main.rs    渲染循环 + 交互 + 透明窗口 + 聊天 UI
src/windows.rs Windows 桌宠窗口平台层(WS_POPUP + 分层透明 + DWM + 置顶 + 拖拽)
src/chat.rs    聊天/语气层(LLM + 语料兜底 + 语气风格注入)
src/chatlog.rs 聊天记录导入 + 语气风格提取
src/lib.rs     库(chat/chatlog 可单独测试)
assets/        丛雨 manifest + 图层 PNG + 语音 OGG + persona + 语料 + CJK 字体 + sovits 数据
docs/          GPT-SoVITS 训练指南
```

## 待办
- [x] GPT-SoVITS 实际训练(GPU 环境, 数据已就绪 `assets/sovits/`)
- [x] TTS 接入 LLM 回复路径(LLM 出文本 → 丛雨语音播放)
- [x] **眨眼/口型动画**(b/e/m 表情层, 孪生映射表驱动)
- [x] 跨平台透明窗口(macOS NSWindow / Linux 合成器, 编译通过待真机)
- [x] **微信/QQ 聊天记录导入**(时间戳文本/HTML/chatlog 工具 JSON)
- [x] **微信聊天记录自动化导入**（对接 chatlog 工具 HTTP API，`PET_CHATLOG_URL` 直连学习）
- [x] **接入自研本地轻量 AI（lyco_chat）**：部署到 CloudStudio 云 GPU（T4），桌宠经 `PET_LLM_BASE_URL` 指向云端预览地址（见 `../docs/lyco-chat-cloud-deploy.md`），本机不跑重负载
- [x] **官方中译语料**：从汉化版 `patch.xp3` 提取 4404 条丛雨中译台词（`murasame_corpus_zh.jsonl`）
- [ ] macOS/Linux 真机验证
