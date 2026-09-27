# Lain42 创作平台：投资顺序与执行清单

2026-09-27。以下是基于已有代码与公开产品的判断，不是市场规模调查、收入预测或已验证付费需求。

## 决策

首个创作平台切口：角色素材工作台。已有 cute-pet 的图层组合经验和 let-gal 的角色/剧情结构可以复用，但现有原生项目仍绑定具体素材，不能宣称任意上传即可安装。先交付可编辑角色，再交付桌宠与游戏适配。

最快获得收入的方向未必是平台首发：已有 Rembg 的垂直商品图交付，以及带人工校验的办公报告服务，更适合验证现金流。角色工作台是现有资产与平台愿景之间的折中，不代表已证明最高收入。

| 方向 | 真实任务/付费理由假设 | 成本与风险 | 决定 |
|---|---|---|---|
| 角色素材→桌宠/视觉小说立绘 | 非程序员能调整自己的角色、预览表情、交付素材 | 素材授权、部件接缝、导出兼容；复用现有项目 | 首发验证 |
| Excel→Word报告/PPT | 周报、经营分析重复劳动；数字可追溯比漂亮模板更重要 | 公式、图表、分页、模板兼容及错误责任 | 并行做人工付费验证，不造办公套件 |
| 商品图批次交付 | 商家重复批量出图、规格检查 | 成熟竞争；需窄行业流程优势 | 现金流候选 |
| let-gal一句话游戏 | 从创意到短篇可玩作品 | 剧情一致性、生图成本、版权、运行时导出；当前无完整浏览器发布证明 | 角色流程后，限定3场景/2角色 |
| 参数化CAD | 定尺寸外壳/面板，减少反复改图 | 单位、公差、几何有效性、制造验收 | 后续窄场景验证 |
| Minecraft皮肤 | 即时看到自己的角色 | 免费替代强，付费低且未验证 | 展示/获客插件 |
| 通用办公生成 | 用户多但微软已有Word/Excel/PPT Agent | 强竞争，格式正确成本高 | 不做首发 |
| 逆向静态分析 | 开发者理解二进制/依赖/符号 | 专业要求、可信结论、工具链维护 | 开发者插件，暂缓旗舰投入 |
| 单图自动Live2D绑定 | 减少拆层/绑定工作 | 遮挡补全、网格、变形质量、SDK许可 | 暂缓；先已有模型预览 |

## 收益判断方式

单次贡献 = 实收 - 模型/生图/API - 存储下载 - 支付费用 - 售后工时成本。
客户端渲染能降低服务器计算费用，但不消除生图费用、移动端内存限制和售后成本。
先邀请3位创作者，每人用授权素材完成一个真实角色，记录完成率、修改次数、耗时、愿付金额。邀请与收费需实际执行，不能把计划算成证据。
实验成功门槛：至少3个独立付费试单、2个再次使用需求、贡献为正；这是投入门槛而非销售预测。

## Skill不是可执行能力

Skill描述任务步骤、输入输出与质量标准；执行器提供工具。每个创作插件需 version、输入schema、权限、执行位置、预览器和导出器。AI返回受限结构化编辑，应用校验后预览，用户可撤销。禁止把下载的skill文本当任意脚本自动执行。
JS先承担图层与UI，WASM按需承担高成本解析/处理；不为使用WASM而增加打包与下载负担。

## TODO（实施者：当前Agent；用户负责实际客户反馈）

- P0 已写：无服务端依赖的PNG图层编辑、撤销、JSON往返、PNG导出。
- P0 验收脚本：GitHub Actions 已覆盖上传、撤销、项目往返、损坏导入保护和手机宽度；每次修改后仍以远端 Action 结果为准。
- P0 正在实现：AI 仅能为现有图层提出位置、缩放、显隐修改；用户先看预览再应用，支持撤销和版本冲突检查。模型只接收描述和图层元数据，PNG 像素留在本机。
- P1 待做：cute-pet导出适配与原生预览对照；不使用商业游戏提取素材作为商用示例。
- P1 待做：let-gal立绘/表情素材适配，再做限定短篇游戏；保留人工编辑。
- P1 待做：真实创作者验证；只在正向证据后增加付费模板/云端生图额度。
- P2 候选：办公报告 skill 小试、限定尺寸参数化 CAD、let-gal 三场景短篇。
- P3 暂缓：完整Blender/CAD/IDE、自动Live2D绑定、通用逆向平台。

## 参考

- 微软已有文件生成Agent：https://learn.microsoft.com/en-us/microsoft-365/copilot/wordexcelppt-agents
- 浏览器AI基础设施而非现成商业产品：https://github.com/huggingface/transformers.js
- 已核实的浏览器逆向方案：https://github.com/radareorg/r2wasm 。ida.js具体项目尚未核实，不据此承诺IDA浏览器版。
- let-gal现有代码：D:/Code/gal/letsgal-ai；当前工作树已有用户改动，本次未改动。

## 2026-09-27 ROI 复核

以下是公开产品与代码的桌面研究，不是 TAM、收入预测或真实用户访谈。2026-09-27 重试后，临时绕过失效代理运行 OMA，网页 grounding 返回 8 条结果，但主要是产品/SEO介绍，没有用户规模或互动指标；Hacker News 为空，Reddit 返回 403，GitHub 抓取返回 422。它能给竞品线索，却没有形成可计量的需求证据。GitHub CLI 当前令牌仍然无效。不能把抓取失败当作没有需求，最终决策仍须用创作者试用和付费测试验证。

同日通过 `curl --noproxy` 对 `https://api.lain42.top/v1/chat/completions` 发送只读 OPTIONS 请求，服务返回 204 并允许 `POST`、`Authorization` 和 `Content-Type` 跨域预检。没有发送 API Key；真实模型 completion、授权及计费仍未验证。

| 方向 | 需求与付费线索 | ROI判断 | 现在怎么做 |
|---|---|---|---|
| AI角色素材可用化 → 桌宠/视觉小说 | Steam 已有桌宠角色编辑和 Workshop；GitHub/网页已有 AI sprite generator、浏览器动画编辑器。社区作者也在处理“生成图看起来像像素画、进游戏却有锯齿、网格与透明背景问题” | 通用生图赛道拥挤；更好的差异点是本地编辑、即时预览、带走项目并适配 cute-pet/let-gal。它复用现有资产，但愿付费仍未知 | **首发验证**已有素材整理、图层预览和可移植导出；生成/拆层不作为首版承诺 |
| Office：Word / Excel / PPT | 微软已把生成代理放进付费 Copilot；真实任务频繁，但格式、公式和数据准确性决定信任 | 更适合 Agent 的动态 skill 和定制报告服务，竞争强，不适合作为创作平台独特旗舰 | 后续做单一“Excel 数据→带来源的 Word 周报/PPT” skill 试点 |
| let-gal 一句话游戏 | 创作演示感强；已有 let-gal CLI/MCP 和游戏引擎 | 角色一致性、剧情修订、图片生成费及试玩发布链路让首版投入偏高 | 复用角色素材工作台；只做 3 场景、2 角色的可玩短篇 |
| Minecraft 皮肤 | 线上免费编辑器很多，出图快但付费差异弱 | 获客小工具，不作独立收费主产品 | 作为导出适配/SEO入口 |
| 参数化 CAD | 合格零件和尺寸变更有较高业务价值 | 客单可能高，但几何正确、单位、公差与制造验收成本高 | 以后只试一个具体品类，不做通用 CAD |
| IDA.js / 静态逆向 | IDA 的 JavaScript 是在 IDA 内部扩展/脚本接口；IDA 软件授权和目标文件授权都需要遵守 | 专业小市场，浏览器并不能因此获得 IDA 分析能力 | 本地只读分析插件候选，不与 WASM 创作旗舰混在一起 |
| 自动 Live2D | 创作者愿付费，但从单张图片自动分层、补遮挡、布网格和变形是高难质量问题；商业 SDK 有发布许可要求 | 研发、验收和许可风险高 | 暂缓自动绑定；先做已有层的预览与导出 |

关键先例：

- [AI Desktop Pet (Steam)](https://store.steampowered.com/app/4227700/AI_Desktop_Pet/) 已将桌宠、角色编辑和 Workshop 分享打包销售。单个商店和 Workshop 只能证明有可售产品与分享渠道，不能证明销售量或市场规模。
- [Sprite Studio](https://github.com/JohnKinyanjui/sprite-maker) 已覆盖 AI 素材、动画、rig、预览和导出，但它是 Tauri 桌面工作流，README 明确不面向 Android/iOS；[SpriteForge](https://github.com/Wilson-Cheng/SpriteForge) 则是完全在浏览器本地运行的 2D rigging 编辑器。已有工具把“生成 sprite”本身做得很广，不能靠一句话生图取胜。
- Reddit 创作者帖指出，AI 生成的“像素画”常有抗锯齿、像素网格不一致和背景清理问题；另有创作者直接表达需要可编辑图层/角色部件、但手工做 layer 与 node 很难。这些是个案线索，不是需求比例：[像素素材清理讨论](https://www.reddit.com/r/aigamedev/comments/1vs4uie/a_free_opensource_tool_that_turns_ai_generated/)；[角色图层工作流讨论](https://www.reddit.com/r/aigamedev/comments/1w2jqj0/i_built_a_tool_that_turns_one_character_image/)。
- [Pixel Refiner](https://github.com/HappyOnigiri/PixelRefiner) 已把网格校正、去背景、裁透明边、减色和 ZIP 导出放进本地浏览器，免费 MIT 发布。这既验证了“生成后可用性”是明确问题，也说明泛用像素清理会迅速商品化；我们的切口应再聚焦到桌宠/视觉小说的可编辑项目与预览导出适配。
- [TaleWeaver](https://github.com/TaleWeaverAdmin/TaleWeaver) 已覆盖 AI 视觉小说流程，但其当前许可证不允许商业服务，故只作竞品观察，不复制其代码。
- [微软 Word/Excel/PowerPoint Agents](https://learn.microsoft.com/en-us/microsoft-365/copilot/wordexcelppt-agents) 属付费 M365 Copilot 功能；[Excel Copilot 文档](https://support.microsoft.com/en-us/excel/copilot/frequently-asked-questions-about-copilot) 也明确提示需要复核 AI 结果。
- [Live2D SDK 发布许可](https://www.live2d.com/en/sdk/license/) 需在发布商业内容前核对；[Hex-Rays JavaScript for IDA](https://hex-rays.com/blog/javascript-for-ida-pro) 是 IDA 内部脚本/插件能力，不等同于可在网页运行的 IDA。
- [Transformers.js](https://huggingface.co/docs/transformers.js/en/index) 与 [sherpa-onnx WASM](https://github.com/k2-fsa/sherpa/blob/master/docs/source/onnx/wasm/index.rst) 是浏览器计算基础设施，不是产品本身。先用 JS 做轻量编辑和预览，确有 CPU/内存瓶颈再把单个重任务移入 WASM。

## ROI 导向的执行顺序

按“近期现金回收 × 现有代码复用 × 可控交付风险”排序，而不是把热度当收入：

1. **近期开单验证**：Rembg 商品图交付与一类 Office 报告服务。两者可先人工校验、按单收费；已有产品/通用任务入口，但当前没有真实订单数据，仍属待验证假设。
2. **旗舰产品实验**：AI 素材可用化工作台。成本可控且复用 cute-pet 与 let-gal，先把用户已拥有或已生成的素材变成可编辑、可预览、能导出的资产。只有创作者完成作品并出现复用/付费，才继续做生图、自动拆层与模板商城。
3. **高客单窄工具**：参数化 CAD 和 IDA 插件需先找到明确行业客户、零件类型或分析任务；在此之前各自做小型技术验证，不搭通用平台。
4. **内容获客插件**：Minecraft 皮肤和免费像素工具可带流量，但已有大量免费替代，先不设为收费核心。

1. **P0（当前开始）**：完成自然语言图层调整；限定为既有图层的 x/y/缩放/显隐。原型由用户填写兼容 API 地址和 key，key 只留在页面内存并发往用户选定服务；不宣称已接入 lain42 账号积分。模型只收到指令与图层元数据，不收到 PNG；拒绝执行脚本，操作先预览、用户确认、可撤销。
2. **P0 已通过远端 CI**：GitHub Actions 的 mock API 浏览器测试覆盖模型请求、确认前不变更、越界拒绝、放弃建议、确认后撤销、PNG 未发送及 Key 不持久化。线上只读 CORS 预检已通过；真实 completion、账号鉴权和积分流程仍需另行验收。CI 绿之前不发布。
3. **P1**：定义版本化、可移植的角色素材包，导出后能由浏览器继续打开；先做 cute-pet 适配，再做 let-gal 立绘/表情适配。
4. **P1**：用户招募 3 名创作者试做真实角色，测完成率、耗时、返工与真实付费意愿；未拿到记录前，不称为 PMF，也不扩大生图算力。
5. **P2**：独立验证 Office 报告 skill。将 Excel 数据生成带来源说明的 Word/PPT，并保留原文件、公式和可人工检查的预览；先服务一类用户。
6. **P3**：Minecraft 仅作引流导出；CAD、Live2D 自动绑定、IDA 插件各自有明确客户与 ROI 证据后再立项。

ROI 原则：对用户的结果负责（可编辑、可带走、可检查），不要按“生成了多少内容”计成功。短期现金流可并行用 Rembg 商品图或 Office 报告做人工小单验证；创作平台首发仍选角色素材工作台。

本次新发现使产品定位收窄：**不是“AI 生图器”，而是跨设备的创作交付层**。图片生成可由用户熟悉的模型提供；Lain42 的价值放在把素材整理成可编辑工程、即时试用，并能无损进入用户选择的桌宠或游戏运行时。若用户只要单张图片，现有生成器足够，不应争抢该场景。

产品价值衡量：用户能否把作品带走并继续修改、是否更快完成真实任务、是否愿意再次使用。插件数量不作为成功指标。
