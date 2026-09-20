# 第三方合规声明 / Third-Party Notices

> 本文件回答两个问题:
> 1. cute-pet 里有哪些东西不是我们写的?
> 2. 每一项的授权状态是什么 —— 尤其是**能不能跟着二进制一起分发**。
>
> 结论见文末「当前能不能发 Release」。仓库自身代码的许可见 [LICENSE](LICENSE)（MIT）。

---

## 1. 代码依赖

### 1.1 vendored Rust crate（`pet/vendor/`，有几处本地改动）

| crate | 版本 | 上游 | 许可证 | 许可证文本 | 我们改了什么 |
|---|---|---|---|---|---|
| `macroquad-ply` | 0.4.14 | [TheRedDeveloper/macroquad-fix](https://github.com/TheRedDeveloper/macroquad-fix) | MIT OR Apache-2.0 | ✅ `pet/vendor/macroquad-ply/LICENSE-{MIT,APACHE}` | 修字符输入队列 LIFO 逆序 bug（issue#1） |
| `miniquad-ply` | 0.4.8 | [TheRedDeveloper/miniquad-fix](https://github.com/TheRedDeveloper/miniquad-fix) | MIT OR Apache-2.0 | ✅ `pet/vendor/miniquad-ply/LICENSE-{MIT,APACHE}` | 新增 HarmonyOS `ohos` native backend（仿 android） |
| `quad-snd` | 0.2.8 | [not-fl3/quad-snd](https://github.com/not-fl3/quad-snd) (crates.io 发布版, sha `5043a949`) | `MIT/Apache-2.0`（仅写在 `Cargo.toml.orig` 的 `license` 字段） | ⚠️ **缺失** | 新增 `src/ohos_snd.rs`（鸿蒙下 noop 桩） |

**⚠️ `quad-snd` 待办**：crates.io 的发布包里没有 `LICENSE` 文件，上游 master 分支也没有
（2026-09-20 核实两条 URL 均 404）。也就是说我们手上这份只靠 `Cargo.toml` 的一句
`license = "MIT/Apache-2.0"` 支撑。作者邮箱在 `authors` 字段里（Fedor Logachev
<not.fl3@gmail.com>），建议发一封邮件要一份正式许可回执，或至少把这条事实记录进 Release notes。

### 1.2 远程 crate 依赖

| crate | 来源 | 许可证 | 备注 |
|---|---|---|---|
| `ply-engine` | crates.io `1.1` → [TheRedDeveloper/ply-engine](https://github.com/TheRedDeveloper/ply-engine) | **0BSD**（仓库 license 为 0BSD） | ⚠️ crates.io 元数据里这个字段是空的，`cargo metadata` / `cargo deny` 会把它判成「未知」。以仓库为准，实质等同公共领域，可放心分发 |
| `lazy-ply` | git `lilyco-42/Lazy-UI` @ main | ⚠️ **仓库无 LICENSE** | 你自己的账号下的仓库 → 没有第三方风险，但 CI 用 `git` 依赖 clone 它，建议顺手给它加一份 MIT，让依赖图干净 |
| 其余 crates.io 依赖（`rust-embed` / `serde` / `ureq` / `anyhow` / `base64` / …） | crates.io | MIT / MIT OR Apache-2.0 | 无修改，按各自许可证 |

### 1.3 Web 引导 JS（`pages/ply_bundle.js`）

`pages/ply_bundle.js`（45 KB，minified）是从 **crates.io `ply-engine 1.1.1`** 的
`js/ply_bundle.js` **逐字节原样拷贝**的。它是该 crate 官方随包分发的浏览器引导
bundle，内含 miniquad `gl.js` + `macroquad_audio` + `sapp_jsutils` + `ply_net`
(HTTP/WebSocket) + `ply_storage`(OPFS) + `ply_fixes` + `ply_accessibility`。

- 来源：`ply-engine 1.1.1` → [TheRedDeveloper/ply-engine](https://github.com/TheRedDeveloper/ply-engine)
- 许可证：**0BSD**（与上表 `ply-engine` 同一份），实质等同公共领域，可放心分发
- 我方改动：无
- 升级方式：升 `ply-engine` 后执行
  `cp ~/.cargo/registry/src/*/ply-engine-<ver>/js/ply_bundle.js pages/ply_bundle.js`

> 为什么必须带上它：`pet/.cargo/config.toml` 用 `--import-undefined` 刻意保留未定义
> 导入；只提供裸 `gl.js` 会让 `audio_*` / `ply_a11y_*` 全部落进"缺失函数桩"并**每帧**
> `console.warn`（实测 ~4400 条/秒，拖垮 DevTools 与主线程）。详见 README「Web 试玩」。

---

## 2. 素材（`pet/assets/`，rust-embed 编译进二进制 → **Release 里是带着它们分发的**）

### 2.1 字体

| 文件 | 来源 | 许可证 | 状态 |
|---|---|---|---|
| `pet/assets/font_wenkai.ttf` | [LXGW WenKai 霞鹜文楷](https://github.com/lxgw/LxgwWenKai) | **SIL Open Font License 1.1** | ✅ 合规所需文本已随仓库：`pet/assets/font_wenkai-OFL.txt`（原样拷贝自上游 `OFL.txt`，未改一字） |

OFL 的两个硬性要求，我们已经满足：

- 字体二进制与许可证文本**一起分发** —— `font_wenkai-OFL.txt` 在 `pet/assets/` 下，
  rust-embed 会把它一起编进二进制；Release 打包时也应把该文件放在同一个目录下。
- **不得改名后分发**： Reserved Font Name 为 `霞鹜` `霞鶩` `落霞孤鹜` `落霞孤鶩` `LXGW`。
  我们把文件重命名为 `font_wenkai.ttf` 属于「使用」而非「再分发改名」—— 但如果
  之后要把它单独作为字体包发出去，必须恢复原名并在包里附这份 OFL。

### 2.2 立绘分层图 ⚠️ 已溯源：柚子社《千恋＊万花》角色版权不归我，商业版须排除（素材走路径 2）

`pet/assets/murasame_layers/` 下 117 个 PNG（约 2.3 MB），配套
`pet/assets/murasame_manifest.json`，其中 `character` 字段是 **`ムラサメ`**，
`name_cn` 为 **「丛雨」**，`canvas` 3600×5100；图层名是日语（`裸` / `髪かぶせ` /
`裸腕差分` …）。配套 `murasame_corpus*.jsonl` 的台词语料为**逐字原作台词**
（`who` 字段同为 `ムラサメ`）。

> 这是一套 **从视觉小说提取的角色立绘分层图 + 台词**，经 `rust-embed` 编进二进制，
> 发布二进制即等于分发素材本身。

**已溯源（2026-09-20，据素材内逐字台词 web 反查确认）**

| 项 | 内容 |
|---|---|
| 出处作品 | 《千恋＊万花》（Senren＊Banka；2016-07-29 首发 PC，2020 上 Steam 中文版，国内由 HIKARI FIELD 发行） |
| 版权方 / 开发商 | 柚子社（Yuzu Soft / YuzuSoft） |
| 角色 | 叢雨 / むらさめ（むらさめ）—— 神刀「叢雨丸」的管理者；CV 佐藤美柑 |
| 人设·原画 | こぶいち（小舞一）・むりりん（夢璃凜） |
| 素材形态 | 从该作提取的角色立绘分层图（117 PNG + manifest）+ 逐字台词 corpus |

**处置结论（模块化：软件商用，角色素材/音频版权不归我）**：软件本体是**模块化**的原创
桌宠框架——**屏幕操控、对话系统、角色渲染引擎等通用能力**，与角色素材**解耦**、版权归我方，
可商业化。但 **丛雨（むらさめ）立绘分层图 + 逐字台词 + 语音音频均取材自柚子社《千恋＊万花》，
版权归柚子社，不归我方**。因此：

- **商业版（付费 / 商用分发）**：**绝不**随商业二进制打包这些角色素材与音频（走「路径 2 对素材」）——
  商业产品只发干净的模块化引擎，角色模块由用户以合法持有的方式自行提供（运行时从本地 /
  私有路径加载，`load_asset` 失败时提示「请放置素材」），我方不在商业包里再分发柚子社素材/音频。
- **角色模块若另行分发**：只能作为**独立、免费、明确标注**的同人二创附加件，声明版权归
  柚子社、不归我方，且**不与商业产品捆绑销售、不作为付费条件**。
- **当前状态**：项目**暂时只做测试**（测试版），测试构建可暂时随包使用上述素材用于内部 /
  免费试玩验证；但一旦进入商用发布，必须按上两条剔除。

> **诚实风险提示（非法律意见）**：柚子社 EULA 通常禁止资源解包与再分发；即便角色模块标为
> 「免费同人二创」，日本同人圈的非正式宽容也不构成法律豁免，且**对商业产品完全不适用**。
> 商业包若误带这些素材/音频即构成未授权商用分发，风险显著。铁律：**商业包 = 无角色素材/音频**。

**可核实内容（技术事实）**

- 角色身份：むらさめ / 丛雨（可选角色模块，版权归柚子社）
- 素材形态：从商业游戏提取的立绘分层图（117 PNG）+ 逐字台词 corpus + 语音音频
- 我方处理链路：提取分层 → 编入 `murasame_manifest.json`（按 dress / diff / layer
  装配）→ `rust-embed` 编进二进制（**仅用于开发 / 测试期**）；商业发布须改为运行时
  按需加载，移出 `rust-embed` 编入范围。
- 软件其余部分（屏幕操控 / 对话 / 渲染引擎等模块化能力）为原创，可商业化；字体 OFL 见 §2.1 合规

**发布要求（必须遵守）**

- 商业 Release：**二进制 / 安装包 / CI artifact 不得含** `murasame_layers/`、
  `murasame_corpus*.jsonl` 及任何柚子社语音音频；Release notes 声明「本商业版不含任何
  第三方版权角色素材与音频」。
- 角色附加件（如有，独立免费分发）：须标注「丛雨立绘/台词/音频取材自柚子社《千恋＊万花》，
  版权归柚子社所有，本附加件为非商用同人二创，不与任何商业产品捆绑」；且不进商业包。

> **v0.2.4 的发布决策（2026-09-20）**：本次 bump 是为了恢复上一轮缺安装包的发布，
> 并且把 OFL 文本一起编进二进制。当时 §2.2 仍为未决；后续（2026-09-20）溯源确认素材
> 来自柚子社《千恋＊万花》，最终定调为**模块化：软件本体（屏幕操控/对话/渲染引擎）原创可
> 商用、与角色素材解耦；角色素材+音频版权归柚子社、商业版须排除（路径 2）、当前仅测试版**。
> v0.2.4 二进制嵌着该素材，属**测试版**用途；一旦进入商用发布须按 §2.2 发布要求剔除，
> 公开 Release notes 须声明版本属性（测试/商业）与版权归属。

### 2.3 其它文本 / 数据素材

| 文件 | 内容 | 状态 |
|---|---|---|
| `pet/assets/characters/*.json`、`*_persona.txt` | 角色人格设定（兰雀 / 飞花） | 自拟内容，无第三方风险 |
| `pet/assets/murasame_corpus*.jsonl` | 台词语料 | ⚠️ 来源同上，见 2.2 |
| `pet/assets/voice/*.srt`、`voice_preset/*.srt` | TTS 字幕时间轴 | 文本时间轴本身风险很低；对应的音频由本机 GPT-SoVITS 合成，**不进仓库**（已被 `.gitignore` 精确忽略） |
| `pet/host_app/**/base/media/*.png` | 鸿蒙 App 图标 | DevEco Studio 模板默认图标，占位用；正式上架前建议换掉 |

### 2.4 平台壳工程

| 目录 | 来源 | 状态 |
|---|---|---|
| `pet/host_app/**` | DevEco Studio / OpenHarmony 模板生成的骨架 | 模板代码，按 HarmonyOS SDK 附带的样例许可使用；业务代码（`src/main/cpp`、`ets`）是我们自己的 |
| `pet/java/**` | Android `OverlayService` / `ScreenCaptureService` | 自写，MIT |
| `pet/gdi_desktop/**` | 备选 Windows GDI 后端 | 自写，MIT |

---

## 3. 当前能不能发 Release

| 发行物 | 结论 |
|---|---|
| 源码 tar / zip（GitHub 自动生成） | ✅ 可以，MIT 已就位；需随包带上 `LICENSE` + 本文件 + `font_wenkai-OFL.txt` |
| 编译好的二进制 / APK / HAR | ⚠️ **测试版**：可随包含 117 张丛雨立绘分层图（取材自柚子社《千恋＊万花》，版权归柚子社、非我方素材）用于功能试玩，须声明「测试版 / 非商用」；**商业版则严禁随包**（路径 2：角色模块由用户合法持有自行提供），否则构成未授权商用分发。其余素材（字体 OFL、自写代码）合规。 |
| 公开的仓库 + CI artifacts | ⚠️ 同上（测试版随包，须声明「测试版 / 非商用」；商业版严禁随包）。注意 CI artifact 与 Pages 部署是公开的，`main` 分支每次 push 都会产出一份带素材的产物；进入商用前须改走 §2.2 路径 2（运行时不编入素材） |

**当前态度**：§2.2 已溯源为柚子社《千恋＊万花》（角色 叢雨 / むらさめ），版权归柚子社、
不归我方。最终定调为**模块化**——软件本体（屏幕操控、对话系统、角色渲染引擎等通用能力）
原创、与角色素材**解耦**、版权归我方、可商业化；**角色素材 + 音频版权归柚子社，商业版必须
排除（路径 2：运行时由用户合法持有自行提供），当前仅做测试版**。铁律：**商业包 = 无角色
素材/音频**。每次公开发版须在 Release notes 声明版本属性（测试/商业）与版权归属（详见 §2.2 发布要求）。
