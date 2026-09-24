//! wasm「异步 LLM 轮询桥」: wasm 无同步网络, `Persona::respond_llm`(ureq)走不了,
//! 此前 wasm(Pages 试玩 / Android 壳)的聊天只能语料兜底 —— AI 对话在壳内是缺席的。
//!
//! 本模块用「JS 调 wasm 导出」的既有模式(同 assets/registry.rs 的
//! cute_pet_alloc / cute_pet_register_asset)打通壳内 LLM:
//!
//! ```text
//! [wasm 帧] 用户输入 ──submit()──▶ pending(请求JSON+原输入)
//! [JS] setInterval 250ms ──poll()──▶ 取走 pending → inflight(读内存拿请求JSON)
//! [JS] fetch OpenAI 兼容端点(可配: ?llm= / localStorage / 壳注入 llm_config.json)
//! [JS] ──resolve(id, ptr, len)──▶ completed(成功文本 或 失败标记)
//! [wasm 帧] take_completed() ──▶ 成功: 显示 LLM 回复; 失败: 走语料兜底(行为不退化)
//! ```
//!
//! 状态机(双平台编译, 可单测); 导出函数仅 wasm。
//! 单 in-flight + 新输入覆盖旧请求: 用户连发多条只处理最后一条(聊天场景合理),
//! JS 无配置/请求失败/超时都 resolve 失败标记 → 兜底, 绝不卡死对话。

/// 一条等待 JS 处理(或已取走等待回复)的请求。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingRequest {
    pub id: u32,
    /// 发给 JS 的完整请求 JSON: {"id":N,"system":"..","history":[[who,text]..],"input":".."}
    pub request_json: String,
    /// 用户原输入(LLM 失败时语料兜底要用)。
    pub input: String,
}

/// 一条已完成的回复: `reply = Some(text)` 为 LLM 成功; `None` 为失败(走兜底)。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompletedReply {
    pub input: String,
    pub reply: Option<String>,
}

#[derive(Default)]
pub(crate) struct BridgeState {
    next_id: u32,
    pending: Option<PendingRequest>,
    inflight: Option<PendingRequest>,
    completed: Option<CompletedReply>,
}

impl BridgeState {
    /// 提交一条新请求。已有 pending/inflight(未完成的旧请求)直接作废 —— 新输入优先。
    pub fn submit(&mut self, system: &str, history: &[(&str, &str)], input: &str) -> u32 {
        self.next_id += 1;
        let id = self.next_id;
        let hist: Vec<serde_json::Value> = history
            .iter()
            .map(|(who, text)| serde_json::json!([who, text]))
            .collect();
        let request_json = serde_json::json!({
            "id": id,
            "system": system,
            "history": hist,
            "input": input,
        })
        .to_string();
        self.pending = Some(PendingRequest {
            id,
            request_json,
            input: input.to_string(),
        });
        self.inflight = None; // 旧请求作废(JS fetch 回来的 resolve 会因 id 不匹配被丢弃)
        id
    }

    /// JS 轮询: 取走 pending → inflight, 返回请求 JSON; 无待处理返回 None。
    pub fn poll(&mut self) -> Option<String> {
        let req = self.pending.take()?;
        let json = req.request_json.clone();
        self.inflight = Some(req);
        Some(json)
    }

    /// JS 回写结果。id 不匹配(请求已被新输入作废)或 JSON 非法 → 静默丢弃。
    pub fn resolve(&mut self, id: u32, reply_json: &str) {
        let Some(inflight) = self.inflight.as_ref() else {
            return;
        };
        if inflight.id != id {
            return;
        }
        let parsed: Option<serde_json::Value> = serde_json::from_str(reply_json).ok();
        let reply = parsed
            .filter(|v| v["ok"].as_bool() == Some(true))
            .and_then(|v| v["text"].as_str().map(str::to_string))
            .filter(|t| !t.trim().is_empty());
        let input = inflight.input.clone();
        self.inflight = None;
        self.completed = Some(CompletedReply { input, reply });
    }

    /// wasm 帧循环消费: 有完成的回复则取走。
    pub fn take_completed(&mut self) -> Option<CompletedReply> {
        self.completed.take()
    }
}

fn state() -> &'static std::sync::Mutex<BridgeState> {
    use std::sync::Mutex;
    static STATE: std::sync::OnceLock<Mutex<BridgeState>> = std::sync::OnceLock::new();
    STATE.get_or_init(|| Mutex::new(BridgeState::default()))
}

// ---------------- wasm 帧循环侧 API ----------------

/// 提交异步 LLM 请求(仅 wasm: 桌面直接走同步 respond_llm)。
#[cfg(target_arch = "wasm32")]
pub fn submit_wasm(system: &str, history: &[(&str, &str)], input: &str) {
    state().lock().unwrap().submit(system, history, input);
}

/// 帧循环消费: `Some((输入, Some(LLM回复)))` 成功 / `Some((输入, None))` 失败走兜底 / None 无。
#[cfg(target_arch = "wasm32")]
pub fn take_completed() -> Option<(String, Option<String>)> {
    state()
        .lock()
        .unwrap()
        .take_completed()
        .map(|c| (c.input, c.reply))
}

// ---------------- JS 调用的导出函数(仅 wasm, 模式同 registry.rs) ----------------

/// poll 数据缓冲: cute_pet_llm_poll 时写入, JS 经 cute_pet_llm_poll_ptr 读走。
/// 复用 Vec 不泄漏; JS 单线程且 poll→ptr→读在同一个 tick, 替换前必然已读完。
#[cfg(target_arch = "wasm32")]
thread_local! {
    static POLL_BUF: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// JS 轮询待处理请求: 返回请求 JSON 的**字节长度**(0 = 无请求), 数据写入 POLL_BUF,
/// JS 再调 cute_pet_llm_poll_ptr() 拿指针读内存。
/// 拆两次调用是因为 JS(f64) 只能精确表示 2^53 内的整数, u64 打包指针+长度会丢精度;
/// 而 wasm32 指针/长度都是 u32 范围, 分开传零风险。
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn cute_pet_llm_poll() -> u32 {
    let Some(json) = state().lock().unwrap().poll() else {
        return 0;
    };
    let bytes = json.into_bytes();
    let len = bytes.len() as u32;
    POLL_BUF.with(|b| *b.borrow_mut() = bytes);
    len
}

/// 上一次 cute_pet_llm_poll 返回非 0 时的数据指针(须在下次 poll 前读完)。
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn cute_pet_llm_poll_ptr() -> *const u8 {
    POLL_BUF.with(|b| b.borrow().as_ptr())
}

/// JS 回写 LLM 结果: {"id":N,"ok":true,"text":".."} 或 {"id":N,"ok":false}。
/// 数据由 JS 先调 cute_pet_alloc 分配再写入(同 register_asset 模式)。
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn cute_pet_llm_resolve(id: u32, ptr: *const u8, len: usize) {
    let raw = unsafe { std::slice::from_raw_parts(ptr, len) };
    let Ok(s) = std::str::from_utf8(raw) else { return };
    state().lock().unwrap().resolve(id, s);
}

// ---------------- 测试(纯状态机, 双平台可跑) ----------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_roundtrip_success() {
        let mut st = BridgeState::default();
        let id = st.submit("sys", &[("user", "你好"), ("ムラサメ", "ん?")], "晚安");
        assert_eq!(id, 1);
        let json = st.poll().expect("poll 应取到请求");
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"input\":\"晚安\""));
        assert!(json.contains("\"system\":\"sys\""));
        assert!(json.contains("ムラサメ"));
        // 已取走, 再 poll 无
        assert!(st.poll().is_none());
        st.resolve(id, r#"{"id":1,"ok":true,"text":"晚安,主人"}"#);
        let c = st.take_completed().expect("应有完成回复");
        assert_eq!(c.input, "晚安");
        assert_eq!(c.reply.as_deref(), Some("晚安,主人"));
        assert!(st.take_completed().is_none(), "取走即清");
    }

    #[test]
    fn resolve_failure_yields_none_reply() {
        let mut st = BridgeState::default();
        let id = st.submit("s", &[], "在吗");
        st.poll();
        // 无配置/HTTP 失败 → ok:false → 兜底
        st.resolve(id, r#"{"id":1,"ok":false}"#);
        let c = st.take_completed().unwrap();
        assert_eq!(c.input, "在吗");
        assert!(c.reply.is_none());
    }

    #[test]
    fn resolve_mismatched_id_dropped() {
        let mut st = BridgeState::default();
        let id = st.submit("s", &[], "第一条");
        st.poll();
        // 请求已被覆盖作废后, 旧 resolve 必须被丢弃(而不是串话)
        st.submit("s", &[], "第二条");
        st.resolve(id, r#"{"id":1,"ok":true,"text":"旧回复串话"}"#);
        assert!(st.take_completed().is_none());
        // 新请求 poll → resolve → 正常完成
        let json = st.poll().unwrap();
        assert!(json.contains("第二条"));
        st.resolve(id + 1, r#"{"id":2,"ok":true,"text":"新回复"}"#);
        let c = st.take_completed().unwrap();
        assert_eq!(c.reply.as_deref(), Some("新回复"));
    }

    #[test]
    fn submit_overrides_unpolled_pending() {
        let mut st = BridgeState::default();
        st.submit("s", &[], "一");
        st.submit("s", &[], "二");
        let json = st.poll().unwrap();
        assert!(json.contains("二"));
        assert!(!json.contains("一"));
    }

    #[test]
    fn malformed_reply_treated_as_failure() {
        let mut st = BridgeState::default();
        let id = st.submit("s", &[], "hi");
        st.poll();
        st.resolve(id, "不是 JSON");
        let c = st.take_completed().unwrap();
        assert!(c.reply.is_none(), "非法回复按失败走兜底");
    }

    #[test]
    fn empty_llm_text_treated_as_failure() {
        let mut st = BridgeState::default();
        let id = st.submit("s", &[], "hi");
        st.poll();
        st.resolve(id, r#"{"id":1,"ok":true,"text":"   "}"#);
        assert!(st.take_completed().unwrap().reply.is_none());
    }

    #[test]
    fn poll_and_resolve_without_submit_are_noops() {
        let mut st = BridgeState::default();
        assert!(st.poll().is_none());
        st.resolve(99, r#"{"ok":true,"text":"x"}"#);
        assert!(st.take_completed().is_none());
    }
}
