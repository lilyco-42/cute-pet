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

### 2.2 立绘分层图 ⚠️ 最高优先级待确认

`pet/assets/murasame_layers/` 下 117 个 PNG（约 2.3 MB），配套
`pet/assets/murasame_manifest.json`，其中 `character` 字段是 **`ムラサメ`（丛雨）**，
图层名是日语（`裸` / `髪かぶせ` / `裸腕差分` …）。也就是说：

> 这是一套 **从商业视觉小说里提取出来的角色立绘分层图**。

这类素材的默认法律状态是「著作权保留」—— 提取行为本身（无论是不是自己买的盘的
资源解包）通常已经在 EULA 的禁止条款里，**再随二进制公开分发是明确的升级风险**。

我不知道也不该替你猜这张图的实际来源渠道（自购提取 / 二创友好官方素材包 / 第三方
转载），所以这里只给判断框架：

| 情形 | 能不能留在公开仓库的 Release 里 |
|---|---|
| 官方明确允许同人二次创作/素材配布的作品 | 通常可以，但要在 Release notes 里标明角色出处与作品名 |
| 自购提取、权利人未表态 | **不建议**。仓库私有即可用；公开 Release 请剔除该目录或改为运行时按需下载 |
| 第三方转载、来源不明 | **不能**。建议立刻从 git 历史里移除 |

**建议动作（按代价从低到高）**

1. 标明出处 —— 在 `README.md` 或 Release notes 里写明角色/作品出处与自己的
   处理链路（这是大多数同人项目的实际做法，也是最便宜的一步）。
2. 若要更保守：把 `pet/assets/murasame_layers/` 从仓库移出，改由首次启动时从 OSS /
   私有链接下载，代码侧 `load_asset` 失败时给出「请放置素材」提示。
3. 彻底清理：走 `git filter-repo` 从历史里剔除（会重写所有 commit hash，务必另开分支）。

请确认来源后回填这一节 —— 在此之前，我把这一项标为 **未决**。

> **v0.2.4 的发布决策（2026-09-20）**：本次 bump 是为了恢复上一轮缺安装包的发布，
> 并且把上一项里缺位的 OFL 文本一起编进二进制。它**不改变**本节的未决状态 ——
> v0.2.4 的二进制里仍然嵌着这 117 张图。决策逻辑：相比已经发布的 v0.2.3，v0.2.4
> 在合规上只增不减（多了 OFL 文本），不存在「越发越糟」；但「应不应该发带这些图的二进制」
> 这一步仍取决于来源确认，已在 v0.2.4 的 Release notes 里写明。等 §2.2 有结论后，
> 若要剥离素材，再走下面 1–3 的其中一种路径发一个新版本即可。

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
| 编译好的二进制 / APK / HAR | ⚠️ 见 §2.2。v0.2.4 的二进制里仍嵌着那 117 张立绘分层图（§2.2 未决）；其余素材（字体 OFL、自写代码）合规 |
| 公开的仓库 + CI artifacts | 同上。注意 CI artifact 是公开的，`main` 分支每次 push 都会产出一份带素材的产物 |

**当前态度**：v0.2.4 已发布，目的是恢复安装包 + 补齐 OFL 文本，合规只增不减；
但「带未决素材的二进制是否应持续公开分发」取决于 §2.2 来源确认。该节一旦有结论，
按 §2.2 末尾的 1–3 路径处理并再发一版即可。在 §2.2 解决前，每次发版请在 Release
notes 里保留一句「丛雨立绘分层素材来源待确认（见 THIRD-PARTY-NOTICES.md §2.2）」。
