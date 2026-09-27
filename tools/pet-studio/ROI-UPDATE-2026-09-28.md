# 创作平台 ROI 复核补充 · 2026-09-28

这是对 `ROI-TODO.md` 的补充记录，不覆盖工作区中已有修改。以下是可公开检索到的产品、仓库和少量社区帖子；它们能验证产品形态和具体摩擦，不能推导市场规模、销量或付费率。

## 本轮重试结果

- 本机 `gh auth status` 仍报告缓存令牌无效；为不把认证故障当成需求证据，改用无凭据的 GitHub 公共 REST 搜索。直连 GitHub API 可用，公开仓库检索成功。GitHub Actions 日志下载接口则要求仓库管理员权限；匿名页面只显示 job 结果，不显示具体日志。
- OMA 在移除失效的 `127.0.0.1:7890` 代理环境变量后完成了 3 轮桌宠制作器窄查询。Reddit 仍返回 HTTP 403；GitHub、Hacker News 和网页 grounding 有响应。结果主要是产品页、教程和少量仓库，不能当作用户需求样本或销售统计。
- 本机和网站都不需要读取浏览器 Cookie；公开页面与 GitHub 公共仓库 API 足够做本轮研究。

## 可核实的任务与替代品

| 方向 | 可观察证据 | ROI 含义与限制 |
|---|---|---|
| 从已有素材制作动画桌宠 | [Shimeji Editor](https://shimejis.xyz/editor) 要用户替换 46 张 128×128 动作图；[NotiSprite Studio](https://briandeking.itch.io/notisprite-studio) 支持导入 PNG/GIF/视频、本机预览、保存，并把分享导出列为 Pro 功能；[Deskie](https://deskie.me/docs/create-a-pet) 声称可从提示词和参考图生成整套动作并逐帧预览；[PetPal](https://petpal.studio/desktop-pet-maker) 声称一次收费 10 美元，先看预览、再下载可移植包。 | “上传素材→拆帧→逐动作预览→导出”有明确产品路径，适合做低服务器成本的本地工具。价格是产品方自述，不是销量或愿付费调查；自动生成桌宠也已有替代品。 |
| 可移植桌宠运行时 | [Desktop Pet Player 的 MIT 仓库](https://github.com/redniu123/pet-player) 定义 `.petpack` ZIP 格式，要求待机 4、行走 6、坐下 4、睡觉 4、互动 4 帧；GitHub 公共 API 在本轮显示 3 stars、2026-09-23 更新。 | 可作为**单一播放器的实验性导出目标**，不是行业标准，受众与生态规模都很小。先生成有效样包、在播放器实测，再决定投入通用运行时。不要把该格式冒充成 cute-pet 已支持的格式。 |
| Minecraft 皮肤 | [Android 编辑器反馈帖](https://www.reddit.com/r/minecraftskins/comments/1qqxrd4/solo_dev_here_built_a_3d_minecraft_skin_editor/) 有用户指出旋转方向、缩放后无法平移到角色边缘等问题；另一开发者帖称现有移动工具广告多、缺少 3D 精细度：[2026 年 8 月反馈](https://www.reddit.com/r/minecraftskins/comments/1w112rw/i_created_a_advanced_layer_extraction_tool_edit/)。 | 痛点集中在触屏编辑精度，但免费替代和新竞品持续出现，没有付费数据。保持为免费演示/获客插件，不单独做收费产品。 |
| 参数化 CAD | [CADAM](https://github.com/Adam-CAD/CADAM) 是活跃的浏览器 text-to-CAD 开源项目；本轮 GitHub API 显示 5,186 stars、GPL-3.0、9 月 18 日更新。 | 用户价值可能高，但通用 prompt-to-CAD 已拥挤，尺寸、公差和可编辑特征树导致高支持成本。仅在找到具体客户后试做一个固定设备外壳。stars 表示开发者关注，不代表购买需求。 |
| Office 文档 | [Microsoft Copilot in PowerPoint](https://support.microsoft.com/en-us/powerpoint/edit-with-copilot-in-powerpoint) 已嵌入既有演示文稿编辑；[用户帖子](https://www.reddit.com/r/powerpoint/comments/1tu6zcd/im_tired_of_ai_tools_that_still_make_me_rebuild/) 抱怨 AI 生成后还要重新整理版式。 | 适合人工交付的单一 PPTX 修复/模板整理服务试单；不应先造 Word/Excel/PPT 通用套件。用户帖子和微软文档都不能说明独立产品的收入空间。 |
| Live2D / 自动绑定 | [Live2D 扩展应用许可说明](https://www.live2d.com/en/sdk/license/expandable/) 要求发布前审核并签署相应许可。 | 自动拆层、补遮挡、布网格和变形本身也有高质量门槛。先做现有素材预览，不把自动 Live2D 绑定作为 MVP。 |

社区帖子只作为摩擦线索：新建工具的作者会自述问题，存在选择偏差；OMA 社区源没有取得足够可复核样本。因此本报告不宣称“市场已验证”。

## ROI 决策与 TODO

1. **现金流先验证 Rembg 人工商品图交付**：用真实付费订单测商家是否愿意为批处理、修边、规格校验和交付付钱。当前没有订单/贡献利润数据，不能称已验证。
2. **创作平台首发选择“已有动作素材的桌宠打包工作台”**，不是通用生图器。第一闭环：在浏览器导入 5 条透明 PNG 横向动作帧带 → 客户端切帧 → 即时播放预览 → 导出包含版本化清单与 PNG 的 `.petpack`。用户可继续使用自己选择的生图 API；首版不托管生图，不产生服务端 GPU 成本。
3. **适配边界**：首个 `.petpack` 仅面向 `redniu123/pet-player` 的 v1 格式，标为实验性兼容。现有 cute-pet 原生程序仍包含 Murasame 专用层和 manifest，不能对外宣称任意素材已经能安装进该程序；后续需要独立的通用运行时 schema 与契约测试。
4. **P1 访谈/试用**：邀请 3 位会用 AI 或手绘角色的创作者，用他们有权使用的素材完成一包动作；记录成功率、用时、返工和是否愿意付费。达到至少 2 人在一周后再次使用且出现 3 个真实付费试单，再考虑生图、云存储或模板商城。样本数是继续投资的门槛，不是行业统计。
5. **P2**：验证现有素材层编辑到 let-gal 立绘/表情的适配；Office 只接一种可追溯的 Excel→Word/PPT 报告任务；CAD 只针对具体设备外壳。
6. **暂缓**：Minecraft 独立收费工具、全功能 CAD/Blender、IDA 网页逆向、自动 Live2D 绑定、AI 一句话完整游戏。没有先拿到稳定用户任务和支付证据，不扩成插件商城或通用平台。

## 本次实施的验证范围

在 `pet-studio` 新增浏览器本地帧带切分、动作预览和 `.petpack` 导出。实现用 Canvas 与 ZIP Store 模式即可完成，不为贴上 WASM 标签增加编译和下载成本；当前并不执行图像生成，也不把 PNG 发往模型/服务器。远端 Actions 将验证手机宽度、动作帧数、包路径与 manifest；在真实播放器导入之前仍标为实验性适配。
