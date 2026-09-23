# Progress

## Current Goal
跨平台桌宠(丛雨): 学习人的语气, 导入喜欢的人的语音/聊天记录, 学习后和用户说话。
已完成: 归档/标注/角色卡/语音库/体积压缩/WASM 版。

## Completed
- [x] 8/31 修复 3 个 issue + 丛雨日语克隆 + 发布 v0.2.2
  - issue#1 输入逆序: macroquad-ply 字符队列 LIFO→FIFO(vendored 进 pet/vendor/ + [patch]), 模拟器验证按序
  - issue#2 语音切换后只有中文: TTS 兜底路径改按语言选库(日→mur001 原声 / 中→greeting)
  - issue#3 日语模式中文回显: 预置问答仅中文模式走 + llm_hint 按语言 + L键/无回复双语文案
  - 丛雨日语克隆突破: 根因=GPT用了过拟合的丛雨微调版; 改用通用GPT(lain-e15)+丛雨SoVITS(音色)→合成内容正确
    (合成「主と一緒にいられて、吾輩は嬉しいぞ」 large-v3 识别「ウェシと一緒にいられて、ワダハイは嬉しいぞ」≈内容全对)
  - 发布: v0.2.2 双 ABI APK(arm64+x86_64) 于 GitHub Release; 三个 issue 均关闭
  - 新增指南: docs/cloudstudio-tts-guide.md(GPT-SoVITS 合成) + docs/troubleshooting.md(已验证问题手册)
- [x] 8/21 功耗修复: 空闲渲染 60fps→10fps, CPU 121%→8%(模拟器)/5%(OPPO 真机)
  - window_conf: Android 开 blocking_event_loop + sleep_interval_ms=100(空闲 10fps,
    交互即时响应) — miniquad-ply 原生支持, 其余平台 cfg 屏蔽不变
  - ScreenCaptureService 截帧节流 4s→30s(视觉 90s/按钮触发足够)
  - 验证: 点击说话音频正常; 悬浮窗/投影正常; 真机 CPU 5%
- [x] 8/21 中日文切换全面日文化: 按钮/全部文本/语音/LLM/视觉回答
  - 面板 UI: 快捷问题/输入框/发送/LLM 提示 按语言切换(QUICK_ZH/QUICK_JP 等)
  - 切换确认语按语言: (日本語モードに切り替えたよ) / (已切换为中文模式)
  - 视觉分析 prompt 按语言(NVIDIA 实测日文输出); 语音/LLM 本就按语言
  - 验证: 视觉模型读屏确认面板已日文化(いる？/ご飯食べた？/言語切替/日本語で入力)
- [x] 8/21 长回复修复: VLM 回复截断 60 字 + 气泡显式换行(ply-engine WrapMode::Words), 防悬浮窗聊天面板溢出
- [x] 8/21 真机回归: OPPO Find X8 (PJD110, Android 16) 全部通过
  - 无线 adb 安装发布版 → appops 授权悬浮窗(ColorOS 接受) → 悬浮窗出现
  - 视觉自动授权(内嵌配置生效) → MediaProjection TYPE_SCREEN_CAPTURE 激活
    → 截帧 → NVIDIA VLM 分析 → 宠物语音念出(audio 44.1kHz, pid 18942)
  - 悬浮窗渲染于 Chrome 之上(粉紫 UI 16%+12% 覆盖深色屏幕)
  - 截图: pet/w3_real_reply.png(已删含个人浏览记录的 consent 截图)
- [x] 8/20-21 W3 权限引导 + 发布架构(模拟器验收通过)
  - 悬浮窗权限引导: 首次启动未授权 → AlertDialog「开启悬浮窗权限」→ 系统设置页
    (ACTION_MANAGE_OVERLAY_PERMISSION) → 返回 Toast 提示手动重启
    (不能同进程自动重启: 旧渲染/音频线程竞争导致 quad-snd panic)
  - 视觉配置 3 级回退: {filesDir} 注入(调试) → 内嵌资产 assets/vlm_config.json
    (发布开箱即用, 已 gitignore) → 环境变量(桌面); 发布版去掉 debuggable
  - 发布版端到端复验: 全新安装 → 授权 → 悬浮窗 → 自动申请录屏 → 截屏 7041B
    → NVIDIA VLM 分析 → 语音念出(全部无需注入/root)
  - 文档: docs/vlm-config.md
- [x] 8/20 W2 视觉链路(MediaProjection 截屏 + NVIDIA VLM"看着你玩游戏") 模拟器验收通过
  - 悬浮窗桌宠(见下条)上加"🔍 看看我在干嘛"快捷按钮 + 配置好视觉模型后自动申请授权
  - 链路: MainActivity 授权 → ScreenCaptureService(FGS, manifest
    foregroundServiceType=mediaProjection, 由 cargo-quad-apk-ply 原生支持)
    → VirtualDisplay 720p 截帧 → JPEG(实测 12581B) → QuadNative.onScreenFrame(JNI)
    → Rust 轮询 take_screen_frame → chat::analyze_screen(OpenAI 兼容,
    NVIDIA NIM meta/llama-3.2-11b-vision-instruct, 实测正确识别 Chrome 屏幕)
    → 丛雨语音+气泡念出分析
  - 配置注入: {filesDir}/vlm_config.json(adb run-as 推送, base_url/api_key/model)
  - 工具链: 无需补丁 — plyx 的 cargo-quad-apk-ply 原生支持
    [[package.metadata.android.service]]; 仅需 Cargo.toml 显式 android_version=36
    + debuggable(供 run-as); 详见 docs/toolchain-cargo-quad-apk-fork.md
  - 截图: pet/w2_vision_reply.png
- [x] 8/19-20 W1 安卓悬浮窗(盖在其它应用上, 含全屏游戏) 模拟器验收通过
  - 渲染表面 SurfaceView → TextureView(修复悬浮窗表面被其它窗口遮挡),
    QuadSurface 挂进 TYPE_APPLICATION_OVERLAY; MainActivity overlay-first
  - 可拖动(QuadDragHandler 拖动窗口/单击互动)、点击说话(音频轨道 44.1kHz)
  - Settings 被系统隐藏属 Android 16 防 tapjacking 安全限制(游戏不受影响)
  - 权限: SYSTEM_ALERT_WINDOW/FOREGROUND_SERVICE/POST_NOTIFICATIONS + minSdk 26
  - 截图: pet/w1_overlay_*.png / w1_final_*.png / w1_relaunch.png
- [x] 8/17 桌宠 LLM 免费模型提示(客户反馈"对话质量差"的配套入口)
  - 聊天面板新增 llm_hint 提示条(lazy-ply chat_panel 组件, 输入框下方居中紫色
    文字): "AI 对话: 免费模型 → NVIDIA NIM · OpenRouter · 商汤 (点击查看)"
  - 未配置 PET_LLM_API_KEY 时显示(配了隐藏); 点击 → 打开 docs/free-llm.md
    (渠道表: NVIDIA NIM / OpenRouter / 商汤 / DeepSeek 备选, 含配置示例)
  - 桌面: cmd start 打开默认浏览器; WASM: console.log "OPENURL:" 前缀,
    build/web/index.html JS 拦截 → window.open 新标签页(v=16)
  - 验证: 桌面窗口显示提示条(紫色长文本 219px 宽, y 693-713 确认);
    点击机制与快捷问题按钮同源(button_id + submitted 拦截, 快捷按钮点击已验证
    UI 有响应); 桌面+WASM 编译通过, 12 个单测全过
  - 说明: 本次只加提示入口, 对话质量三项修复(上下文/IDF 检索/预置收紧)在上一项
- [x] 8/17 桌宠对话质量修复(客户反馈: 中文怪/前言不搭后语/误判)
  - 根因1 前言不搭后语: respond_llm(&[], &input) 传空历史 → 改传最近 10 轮
    (chat_state.history 旧→新, 角色名映射 assistant/user)
  - 根因2 语料兜底答非所问: respond_corpus 用输入字节哈希随机取(4404 条抽 1)
    → 改 IDF 加权连续子串检索(构建时统计语料字符逆文档频率, 罕见字命中高分,
    虚词"了/吗/你"自动降权), 命中过低回退首条通用台词
    - 验证: "吃饭了吗"→"你也要好好吃饭哦" "我喜欢你"→"吾辈喜欢你这个男人"
      "我回来了"→"吾辈回来了哦" 精准命中
  - 根因3 预置问答误判: input.contains(kw) 宽匹配 → preset_match() 统一逻辑:
    否定前缀排除("不想你"不误中"想我了吗") + 多条命中取更长关键词 + 忽略标点
    (关键词带"?"可匹配不带"?"的输入, 修了"想我了吗"无问号永不命中的隐藏 bug)
  - Persona 增加 char_idf 字段(语料构建时统计, 内存 ~几 KB)
  - 测试: chat.rs 12 个单测全过(preset_match 否定/标点/优先级用例 + 检索质量用例)
  - 遗留: 语料是游戏台词非对话语料, 问候/天气等意图无对应台词→回退首条(可后续补语料)
- [x] 8/17 和风背景 lazy-ply 组件化(高内聚低耦合)
  - lazy-ply 新增 components/background.rs: pet_background(now,w,h) 直绘组件
    渐变+圆月+云+花瓣, 配色走 assets/components/background.toml(可配置)
  - config.rs 加 PetBackgroundConfig(component_config! 宏 + 样式表测试)
  - main.rs 删内联实现, 改一行调用 demo::components::pet_background(cfg android)
  - 分步验证: 矩形(渐变)+圆(月亮) ✓ → 椭圆(云/花瓣, 配置开关 cloud_enabled/
    petal_enabled 默认关→开) ✓ 像素 diff 确认渲染
  - 云/花瓣在浅背景上偏淡(装饰级), 可调 toml 颜色/alpha 增强
- [x] 8/17 预设语音 24k→44.1k 修复"吃饭了吗/想我了吗"卡死
  - 根因: voice_preset/feiXX.ogg 是 24kHz, quad-snd 最近邻重采样后数据
    opensles 播放异常(卡主线程/无输出) → 全部转 44.1kHz
  - 另: preset_sounds 改 Vec<Option<Sound>> 按 idx 对齐(加载失败占位, 防错位)
  - F3 调试面板(Android 不映射, 桌面可用): 显示 preset/cn/jp 加载数
- [x] 8/17 发布 v0.1.1: pet/release/cute-pet-v0.1.1-arm64.apk (8.18MB arm64)
  - 语音 44.1kHz(消除 quad-snd 重采样电音) + 互斥播放(防重叠, 连点验证 1 track)
  - 版本号 0.1.1, INTERNET+cleartext 权限, 发布说明 pet/release/README.md
  - 残留: 模拟器链路轻微电音(真机待验); QEMU 音频会话周期性断(重启模拟器恢复)
- [x] 8/17 桌宠 5 项交互修复(模拟器验证通过)
  - ① 点按钮没语音: 根因=APK 缺 INTERNET 权限(cargo-quad-apk 键名 permission 单数
    + application_attributes) + CloudStudio TTS 服务 403 已死 + Android 无 ffmpeg
    → 加权限/明文, TTS 失败或超时自动播内嵌兜底语音(点击/回复必有声音)
  - ② 语音表情不同步: VOICE_META 表(文件名→表情face+中日台词), 从 manifest 图层名
    解码 9 个表情语义(01默认 02微笑 03发懵 04惊讶 13困扰 14生气 19孩子气 20/21不满),
    点击/说话按表同步表情, 表可直接编辑
  - ③ 点击台词显示: 角色头顶台词气泡(measure_text+draw_text_ex, 文楷字体, 中日随语言)
  - ④ 音量键存疑: 根因=指示条画在 y=10 被状态栏盖住 + touches() Started 判断在
    DOWN+UP 同帧到达时丢失 → 改鼠标释放路径(上半屏边缘=音量/角色区=说话) + 指示条 y=90
  - ⑤ 语言选择: 🌐切换语言快捷按钮 + L 键, 中文/日本語 模式; 中文语料过滤假名
    (67 条垃圾日语剔除), 日文模式用日文语料; LLM 提示词按语言; 点击语音/台词随语言
  - 截图: pet/t2_button.png(按钮回复+兜底语音) t4c_volume.png(音量指示条)
    t5_lang1.png(语言切换) t5_lang2.png(日文台词气泡)
- [x] 8/17 台词气泡放大: 字号/内边距随 ui_scale 缩放(Android 20px→54px) + 长文本
  自动换行(wrap_text + draw_multiline_text_ex), 文字带高度 15px→166px 区域
- [x] 8/17 模拟器无声修复: 根因=模拟器默认音频后端把 MUSIC 流固定衰减到
  G db=-33dB(≈2%, 系统层, stagefright 对照证实非 app 问题) → 重启模拟器带
  `-audio host` 后 -14dB 可闻(见环境备忘)
- [x] 8/17 模拟器 UI 布局回归验证通过(medium_phone x86_64 + 宿主 GPU)
  - APK 改双 ABI 构建(aarch64 + x86_64), 装进 x86_64 模拟器运行
  - 修复 EGL 崩溃: miniquad 默认 sample_count=1 在 AMD GPU 透传下
    eglChooseConfig 匹配不到 config(cfg_count=0 panic) → Conf sample_count=0
  - 修复编译错误: KeyCode::LBracket/RBracket → LeftBracket/RightBracket
    (miniquad-ply 枚举名, 8/16 音量快捷键新增代码首次编译才发现)
  - 布局逐带对比 8/15 已验证截图: 角色居中(880-1664)/两行快捷按钮
    (1892-2156)/输入框(2164-2328) 完全一致; 点按钮出气泡; 点角色有音频输出
  - 截图证据: pet/emu_layout_test2.png(主界面) emu_layout_test3.png(气泡后)
- [x] 8/17 修复 vision-toolkit 运行时不可用(uv 装 pillow 失败)
  - 根因1: 全局 uv.toml 默认索引=清华源, 当前对请求 403 → 改阿里云默认+pypi.org 兜底
  - 根因2: requirements.lock 锁定 numpy==2.5.1 要求 Python>=3.12, 但引导
    `python` 解析到 uv shims 的 3.11.15 → settings.yaml 显式配置
    vision-toolkit.runtime.python 指向 3.13.14
  - 验证: 3.13 venv 完整装齐 pillow12.3.0/numpy2.5.1/vtracer0.6.15,
    vision_crop 端到端通过, 运行时 marker py313 就位
- [x] Android UI 缩放修复: miniquad dpi_scale=1 导致按物理像素 1:1 渲染(UI 过小)
  - ui_scale = screen_width/400dp, 统一放大按钮(48dp 胶囊填充)/输入框(60dp)/气泡文字(18dp)
  - chat_panel 新增 quick_columns(快捷问题 3 列两行) + bubble_font_size 配置
  - 点击互动区按 ui_scale 缩放(避开大控件)
  - 模拟器验证: 气泡顶部/角色居中/2x3 大按钮/输入框 全部正常
- [x] Android APK 编译成功(plyx apk --native)+ 模拟器验证
  - 修复: lazy-ply 无操作 build.rs + shader-build 依赖删除(spirv-cross 交叉编译 host 链接失败)
  - 修复: aapt2 无法处理中文文件名 → metadata.android assets 指向空目录 assets_apk/(全部资产经 rust-embed 进 .so)
  - 修复: chat_demo CLI bin 移入 examples/(cargo-quad-apk 会为每个 bin 出 APK)
  - 产物: pet/target/android-artifacts/release/apk/cute-pet.apk (6.18MB, arm64-v8a)
  - 模拟器 medium_phone 验证: 角色居中 + 快捷按钮 + 输入框正常渲染
  - 角色定位改为每帧按实际屏幕尺寸计算(桌面/Android/WASM 自适应)

## Active TODO
- [ ] macOS/Linux 真机验证桌宠平台层
- [ ] 浏览器验证 wasm 桌宠(http://127.0.0.1:8899 已起服务)
- [ ] 桌宠增强: 向量记忆 / wasm 版预置语音(需 JS 侧传文件)
- [ ] GDI 桌宠: 聊天 UI 继续完善(输入光标/多轮历史) / 音量键 / 拖动窗口

## Completed
- [x] 8/17 GDI 桌宠 Windows 版: 内存 394MB→2.36MB + 体积 3.1MB→1.1MB
  - 内存链路: macroquad(394MB) → GDI 直渲(2.83MB) → 加聊天后 4.24MB → 优化
    到空闲 2.36MB(Private), 交互后 winmm 播放完成自动卸载回落
  - 修复 3 个关键 bug:
    ① UpdateLayeredWindow ptDst 传 {0,0} 会把窗口拉回屏幕原点 → 改传 NULL
       保留 SetWindowPos 位置(此前误判"窗口在 (0,0)"实为 DPI 125% 缩放)
    ② 眨眼状态机: if>=3000{BLINK} else if>=3120{NORMAL} 顺序错误导致 BLINK
       分支永远先匹配、永不恢复 → 改"先判恢复再判触发"
    ③ 输入区空文本 DrawTextW 崩溃(USER32 0xc0000005, 空 Vec as_ptr 悬垂)
       → text_utf16 非空才 DrawTextW; 聊天 1 轮即崩 → 全流程通过
  - 语音动态加载: PlaySoundW 改 LoadLibrary+GetProcAddress(启动不载 winmm),
    播放结束 6s 后 FreeLibrary 卸载音频栈(winmm/winmmbase 可卸,
    MMDevAPI/AUDIOSES 系统 DLL 残留 ~0.5MB 不可控)
  - 体积: voice 2.4MB→624KB(MS ADPCM 4:1, PlaySound 原生支持验证) +
    exe UPX 160KB→75KB → 发布包 pet.bmp 450KB+eye_patch 10KB+voice 624KB
    +exe 75KB ≈ 1.1MB
  - 验证: 点击说话(ADPCM 语音轮播)/聊天回复气泡/眨眼 3s 周期 全通过
  - 遗留: 点击交互时内存瞬时 ~3.4-3.6MB(winmm 加载), 播完回落到 ~2.4MB;
    UPX 版若杀软误报可退回 raw exe(gdi-pet-raw.exe 160KB)
- [x] 8/17 GDI 桌宠资源内嵌单文件化
  - 根因: 直接双击 exe(cwd=exe 目录)时 assets 相对路径失效 → 无声音 + 
    眨眼空补丁越界崩溃(eye_patch 空 Vec) → 进程秒退
  - 修复: include_bytes! 嵌入全部资源(pet.bmp 450KB + eye_patch 10KB +
    10 个 ADPCM wav 624KB), 任意 cwd 启动均可; load_bmp 改内存解析
  - 语音改 SND_MEMORY 播放嵌入 wav(无需文件路径, 数据 'static 安全)
  - 单文件: raw 1.21MB → UPX 555KB; 验证: C:\Windows cwd 启动存活/立绘
    渲染/点击 winmm 加载播放→10s 卸载/聊天 全通过
  - 内存: 嵌入后空闲 3.15-3.28MB(嵌入静态数据 ~0.9MB 进映像, 略升)

## Completed
- [x] 布局优化: 对话框图层不再覆盖立绘(角色缩小居中 + 上下分区)
  - 窗口=舞台: 顶部气泡区 + 中部立绘 + 底部控件区(按钮+输入框), 互不重叠
  - 角色渲染缩放 0.9 并居中(实测立绘比 bbox 低 ~100px, 经验上移 25px 校准)
  - chat_panel max_bubbles 6->3(紧凑); 点击互动区与控件区对齐
  - 验证: PET_VERIFY 截图 角色[190,730] 按钮750 输入框800 全分离; release 6.09MB(UPX); wasm 7.17MB ?v=7
- [x] lazy-ply 集成: 宠物聊天 UI 改用 lazy-ply 组件库(1 宿主组件 + chat_panel 复用)
  - lazy-ply 新增 chat_panel 组件(气泡历史 + 快捷问题 + 输入框, 样式走 toml 级联) + demo 集成
  - cute-pet 依赖 lazy-ply(path dep), 立绘/窗口/动画保留 ply-engine 直绘, 聊天 UI 换成组件
  - 桌面验证: 透明窗口/立绘/按钮/输入框正常(PET_VERIFY 截图)
  - wasm 重出: 6.58 -> 7.17MB raw / 4.51 -> 4.70MB gz(引入 a11y/text-styling/fontdb), 部署 ?v=6
- [x] WASM 版重出: app.wasm 73.77 -> 5.8MB (-92%), gzip 3.99MB
  - release-wasm profile(无 lto, opt-level=s) + .cargo/config.toml --import-undefined
  - build/web/: app.wasm 5.8MB + app.wasm.gz 3.99MB + ply_bundle.js 44KB + index.html
- [x] 体积压缩: 桌面 exe 77 -> 5.19MB (sovits移出/字体2MB/png量化/UPX)
- [x] 语音库: 飛花/兰雀 各 75 条 (150 条)
- [x] 角色卡 + PET_CHARACTER 切换 + PET_VOICE_DIR 播放
- [x] 台词标注 24878 条 + 归档 voice_index.csv

## Backlog Ideas
- [ ] wasm 版预置语音: 通过 JS 侧 fetch 注入语音(绕过无文件系统)
- [ ] 剩余 mur 语音 ASR 补标
- [ ] 全角色语音素材库微调

## Blocked
- 无

## 环境备忘(2026-08-17)
- 模拟器必须带 `-audio host` 启动: 默认音频后端会让 MUSIC 流固定衰减到
  G db=-33dB(≈2% 音量, 基本无声); 显式 `-audio host` 后为 -14dB(可闻)。
  - 启动: emulator -avd medium_phone -no-boot-anim -no-snapshot-save -audio host
  - 判定: adb shell "dumpsys media.audio_flinger | grep -A2 'Tracks of which'" 看 G db
  - 真机无此问题(真机增益由系统音量正常控制)
- 模拟器长时间运行后 QEMU→Windows 音频会话可能中途断开(app 播放正常但宿主无声,
  -14dB/轨道 active 都正常) → 无声时重启模拟器即可恢复(-audio host)