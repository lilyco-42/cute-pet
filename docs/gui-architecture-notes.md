# GUI 框架架构调研笔记

> 目的: 为 lazy-ply(立即模式组件库)与 pet(桌宠)的架构演进提供参考。
> 结论先行: **不迁移、只借鉴**。以下笔记是两轮调研(跨语言主流 + Rust 系)的精华提炼。

## 1. 三大思想流派

### 1.1 立即模式 (Immediate Mode) — 我们所在的世界
- 代表: Dear ImGui (C++, 75k★)、egui (Rust, 30k★)、lazy-ply
- 核心: 每帧从状态重建 UI, 无回调、无引用; "GUI 显示的就是最新状态", 永不脱钩
- 已知短板(egui 官方文档亲述):
  - **布局悖论**: 居中弹窗需要"先知道尺寸才能定位, 但定位前必须布局"。egui 解法:
    两遍 pass 或"用上一帧尺寸"(代价: 首帧布局延迟/闪烁)
  - **大滚动区**: 全量布局每帧重跑会慢 → 只布局可见区
  - **依赖图膨胀**: 低复杂度 UI 极简; 复杂度升高后状态全局化 + 手动缓存反噬维护性
    (Tritium 团队因此从 egui 迁去 slint)
- 优点(egui 官方): 交互密集时反而比 retained 更快; 空闲不重绘 → CPU 省
- 教训: lazy-ply 已自带 idle pacing + Id 系统, 方向正确。避免大滚动区全量重布局。

### 1.2 Elm / 受控单向数据流 (Retained + Message)
- 代表: Elm、Iced (Rust, 31k★)、Redux、Avalonia MVVM (C#, 31k★)
- 核心: **Model / View / Update 三分**; 交互 → 类型化 Message → 纯 update() 返回新状态
- 价值:
  - update 是纯函数 → **可单测、可回放、时间旅行**
  - 状态集中, 消灭隐式时序
- 成本: 每帧重建 immutable model 依赖 GC/拷贝 → 在 Rust 需折中(iced 为它牺牲了简洁)
- 教训: 桌宠是 15fps 游戏式 UI, 全盘 Elm 化是负优化; **取分层, 保留立即模式**。

### 1.3 细粒度响应式 (Fine-grained Reactivity)
- 代表: Jetpack Compose (Kotlin, 19k★)、Dioxus Signal (Rust, 38k★)、Makepad Live (6.5k★)
- 核心: 状态 → 依赖图, 写时只通知**真正读取它的那部分 UI**
- 关键实现:
  - **Compose Snapshot (MVCC)**: 每次写插入 StateRecord, 帧首 `sendApplyNotifications()`
    批量应用+通知。**跨线程无锁安全**(隔离快照), 批量写 → 一次重组
  - **Compose SlotTable**: 组合树存进位置化 slot(flat array + gap buffer, 已改链表),
    `remember` 是"按调用位置读 slot" → 状态随位置记忆
  - **Dioxus Signal (generational-box)**: `Signal<T>` 永远 Copy(即使 T 非 Copy),
    零 unsafe; `Send+Sync` 信号可直接搬进后台线程; 只读即订阅
  - **Makepad Event/Action/Signal 三层**: 底层事件(输入) → 组件 Action(业务) →
    跨线程 Signal+Channel; `Arc<AtomicBool>` + 主线程 action 队列收割

## 2. 渲染架构对照

| 框架 | 分层 | 渲染 |
|---|---|---|
| Flutter (178k★) | Widget→Element→RenderObject | 自研引擎(Impeller) |
| Slint (23k★) | DSL 编译期优化 + 单内存区 | femtovg/skia/software 可选 |
| Makepad | 保留模式 widget + Live DSL + SDF shader | 自研 GPU |
| Xilem (5.4k★) | View 树 diff → Masonry element 树 | wgpu (Vello) |
| Dioxus | VirtualDOM diff + 模板静态跳过 | WebView/native |
| Compose | SlotTable + Snapshot | Skia |

- Flutter 三层核心: **声明(Widget) 与 实例化(Element) 分离**, 声明可丢弃、实例保留状态
- Xilem: **View 树(轻量, 可弃) + 保留 element 树 + diff** — 纯立即与纯 retained 的中间态;
  `Memoize` 剪枝、`lens` 切片子状态
- Compose 教训: **结构变更(增删节点)比属性变更贵** → 别在热路径改树结构

## 3. 事件 / 通信架构对照

| 框架 | 机制 | 特点 |
|---|---|---|
| Qt | Signal/Slot | 类型安全、松耦合; 发射方不知接收方 |
| Makepad | Event/Action/Signal | 三层, 跨线程用 Signal+Channel |
| Compose | Snapshot 帧首批量应用 | 无锁跨线程 |
| Dioxus | Signal 读写自动订阅 | 读取即订阅, 零成本 |

- 共同收敛点: **状态集中 + 细粒度订阅 + 帧首批量收割**

## 4. 对 lazy-ply / pet 的可落地结论

### pet(立即模式桌宠, 1215 行 main.rs)
1. **状态集中**: ~30 个散落局部变量 → 单一 `AppModel` struct(Elm Model 精神, 不搬架构)
2. **类型化事件**: `if is_key_pressed(...)` 瀑布 → `enum AppEvent` + 统一 update()
3. **异步帧首收割**: TTS/LLM 后台线程结果 → 帧首 `take()` 成事件(替代散落 Mutex 轮询;
   Makepad/Compose 同款思路)
4. 不做: 不可变 model、QML 绑定、VDOM diff —— 对 15fps 游戏式 UI 是负资产

### lazy-ply(组件库)
1. **Id 系统**: 保持"组件状态位置化"(= Compose SlotTable 直觉); 稳定 id 命名约定写进文档
2. **组件返回裸值**: 已正确; 可选加 Message 化, 非必需
3. 结构变更注意: 列表/条件渲染别在每帧重建子树(Compose 教训)

## 5. 参考来源

- Elm Architecture Guide — guide.elm-lang.org/architecture
- egui README(为什么立即模式 + 布局悖论) — github.com/emilk/egui
- Iced README(Model/Message/View/Update) — github.com/iced-rs/iced
- Xilem ARCHITECTURE.md — github.com/linebender/xilem
- Makepad Book(Event/Action/Signal + Live DSL) — makepad.rs
- Compose Snapshot 系统 — blog.zachklipp.com / developer.android.com(phases)
- Compose SlotTable 内部 — mukuljangra.com / doveletter.dev
- Dioxus Signals & 架构笔记 — github.com/DioxusLabs/dioxus
- Rust GUI 2025 调查 — boringcactus.com; Wren Learns Rust; Tritium 博客
- Slint 架构(编译器 + 多后端) — github.com/slint-ui/slint
