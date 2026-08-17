//! 聊天记录导入 + 语气风格提取。
//!
//! 需求: 「导入喜欢的人的语音/聊天记录, 学习它」。这里负责导入端:
//!   1. 读取导出的聊天记录(JSON/JSONL/纯文本)
//!   2. 筛出目标说话人的消息
//!   3. 提取语气特征: 平均长度/短句比例/emoji/口头禅/代表句
//!   4. 生成可注入 LLM persona 的风格描述片段
//!
//! 支持格式:
//!   - JSON:  `[{"from":"名字","text":"..."}]` 或 `{"messages":[...]}`
//!   - JSONL: 每行一个 `{"from":..., "text":...}`
//!   - 纯文本: `名字: 消息`(每行一条)

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StyleProfile {
    pub speaker: String,
    pub msg_count: usize,
    pub avg_len: f32,
    pub short_msg_ratio: f32,
    pub emoji_count: usize,
    pub question_ratio: f32,
    pub common_phrases: Vec<String>,
    pub samples: Vec<String>,
}

/// 从聊天记录文件提取某说话人的语气特征。
pub fn extract_style(log_path: &str, speaker: &str) -> anyhow::Result<StyleProfile> {
    let raw = std::fs::read_to_string(log_path)?;
    let msgs = parse_messages(&raw)?;
    extract_style_from_messages(&msgs, speaker)
}

/// 从 (说话人, 文本) 消息列表提取某说话人的语气特征。
/// 与 `extract_style` 共用统计逻辑, 便于对接 chatlog API / 内存数据等数据源。
pub fn extract_style_from_messages(msgs: &[(String, String)], speaker: &str) -> anyhow::Result<StyleProfile> {
    let mine: Vec<&str> = msgs
        .iter()
        .filter(|(who, _)| who.trim() == speaker)
        .map(|(_, t)| t.as_str())
        .collect();
    if mine.is_empty() {
        anyhow::bail!("没有找到说话人 '{speaker}' 的消息(共 {} 条)", msgs.len());
    }

    let total: usize = mine.iter().map(|t| t.chars().count()).sum();
    let avg_len = total as f32 / mine.len() as f32;
    let short = mine.iter().filter(|t| t.chars().count() <= 4).count();
    let questions = mine.iter().filter(|t| t.trim_end().ends_with('？') || t.trim_end().ends_with('?')).count();
    let emojis = mine.iter().map(|t| t.chars().filter(|c| matches!(c, '😀'..='🙏' | '🥰' | '❤' | '✨')).count()).sum();

    // 高频词/口头禅: 统计 2-3 字子串出现次数
    let mut freq: HashMap<String, usize> = HashMap::new();
    for t in mine.iter() {
        let chars: Vec<char> = t.chars().collect();
        for w in 1..=3usize {
            if chars.len() <= w {
                break;
            }
            for i in 0..=chars.len() - w {
                let sub: String = chars[i..i + w].iter().collect();
                *freq.entry(sub).or_default() += 1;
            }
        }
    }
    let mut phrases: Vec<(String, usize)> = freq.into_iter().filter(|(s, c)| *c >= 3 && s.chars().count() >= 2).collect();
    phrases.sort_by(|a, b| b.1.cmp(&a.1));
    let common: Vec<String> = phrases.into_iter().take(12).map(|(s, _)| s).collect();

    // 代表句: 取长度适中(8-30字)的样本若干
    let mut samples: Vec<String> = mine
        .iter()
        .filter(|t| (8..=30).contains(&t.chars().count()))
        .take(8)
        .map(|s| s.to_string())
        .collect();
    if samples.is_empty() {
        samples = mine.iter().take(5).map(|s| s.to_string()).collect();
    }

    Ok(StyleProfile {
        speaker: speaker.to_string(),
        msg_count: mine.len(),
        avg_len,
        short_msg_ratio: short as f32 / mine.len() as f32,
        emoji_count: emojis,
        question_ratio: questions as f32 / mine.len() as f32,
        common_phrases: common,
        samples,
    })
}

/// 把语气特征渲染成一段可注入 persona 的 prompt 片段。
pub fn style_prompt(p: &StyleProfile) -> String {
    let mut s = format!(
        "\n\n【学习目标对象: {}】请模仿以下说话风格:\n- 平均消息约 {:.0} 字, 短句(≤4字)占比 {:.0}%, 提问倾向 {:.0}%{}\n",
        p.speaker,
        p.avg_len,
        p.short_msg_ratio * 100.0,
        p.question_ratio * 100.0,
        if p.emoji_count > 0 { format!(", 常用 emoji(累计 {} 个)", p.emoji_count) } else { String::new() },
    );
    if !p.common_phrases.is_empty() {
        s.push_str(&format!("- 口头禅/高频词: {}\n", p.common_phrases.join("、")));
    }
    s.push_str("- 说话风格示例:\n");
    for (i, sample) in p.samples.iter().enumerate() {
        s.push_str(&format!("  {}. {}\n", i + 1, sample));
    }
    s
}

/// 从 chatlog 工具(sjzar/chatlog 及其 fork)HTTP API 拉取聊天记录并提取语气。
/// API: `GET /api/v1/chatlog?talker=<名字>&format=json` → `{"messages":[...]}`。
/// 需先在本地跑起 chatlog 服务(`chatlog` 启动 + 开启 HTTP 服务, 默认 5030 端口)。
/// 非 WASM(需要网络)。
#[cfg(not(target_arch = "wasm32"))]
pub fn extract_style_from_chatlog(base_url: &str, speaker: &str) -> anyhow::Result<StyleProfile> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/v1/chatlog?talker={}&format=json&limit=500", urlencode(speaker));
    let mut resp = ureq::get(&url).call()?;
    let raw = resp.body_mut().read_to_string()?;
    let msgs = parse_messages(&raw)?;
    if msgs.is_empty() {
        anyhow::bail!("chatlog API 未返回消息({url})");
    }
    extract_style_from_messages(&msgs, speaker)
}

/// 简单 UTF-8 百分号编码(与 chat.rs 一致, 避免跨模块重复实现)。
#[cfg(not(target_arch = "wasm32"))]
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 解析消息为 (说话人, 文本) 列表。
/// 自动识别格式:
///   - JSON:  `[{"from":..., "text":...}]` / `{"messages":[...]}`
///   - HTML:  微信/QQ 导出页 `<div class="msg">…<span class="name">…` 或 `<b>名字</b> 内容`
///   - 时间戳文本:  `YYYY-MM-DD HH:MM:SS 名字\n内容`(微信/QQ 电脑版导出)
///   - 纯文本:  `名字: 内容`(每行一条, 容忍全角冒号)
fn parse_messages(raw: &str) -> anyhow::Result<Vec<(String, String)>> {
    let trimmed = raw.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        return parse_json(trimmed);
    }
    if trimmed.contains("<div") || trimmed.contains("<span") || trimmed.contains("<p") || trimmed.contains("<b>") {
        return Ok(parse_html(trimmed));
    }
    if let Some(out) = parse_timed_text(trimmed) {
        return Ok(out);
    }
    Ok(parse_simple_text(trimmed))
}

/// JSON: 兼容微信/QQ 备份常见字段名, 以及 chatlog 工具(sjzar/chatlog)的 API 格式。
fn parse_json(raw: &str) -> anyhow::Result<Vec<(String, String)>> {
    let v: serde_json::Value = serde_json::from_str(raw)?;
    let arr = match v {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(o) => o
            .get("messages")
            .or_else(|| o.get("msg"))
            .or_else(|| o.get("chatLog"))
            .and_then(|m| m.as_array())
            .cloned()
            .unwrap_or_default(),
        _ => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for m in arr {
        // chatlog 工具: 私聊 sender 为空、群聊 sender 为发言者; talker 为会话对象
        let talker = m["talkerName"].as_str().or(m["talker"].as_str()).unwrap_or("").to_string();
        let sender = m["sender"].as_str().unwrap_or("").to_string();
        // 纯文本消息(type=1); 其余(图片/语音/链接)内容可能为空或非对话文本
        let is_text = m["type"].as_i64().map(|t| t == 1).unwrap_or(true);
        let who = if !sender.is_empty() {
            sender
        } else if !talker.is_empty() {
            talker
        } else {
            m["from"].as_str().or(m["name"].as_str()).or(m["user"].as_str())
                .or(m["nickname"].as_str()).or(m["speaker"].as_str())
                .unwrap_or("").to_string()
        };
        let text = m["content"].as_str()
            .or(m["text"].as_str())
            .or(m["msg"].as_str())
            .or(m["message"].as_str())
            .unwrap_or("").trim().to_string();
        if !text.is_empty() && is_text {
            // chatlog 里 isSelf=true 表示自己发的消息(学习对象通常是对方, 但保留字段)
            out.push((who, text));
        }
    }
    Ok(out)
}

/// HTML 导出(微信/QQ 网页版保存): 提取每段消息的说话人 + 文本。
/// 兼容:
///   <div class="msg"><span class="name">小美</span> <span class="time">18:26</span> 你到家了嘛？</div>
///   <p><b>小美</b> 你到家了嘛？</p>
///   <div class="message"><div class="sender">小美</div><div class="content">…</div></div>
/// 策略: 逐标签扫描, 遇到说话人标记取名字; 时间/日期等元数据标签整段跳过;
/// 其余文本节点在「当前有说话人」时累积, 遇到块级闭合标签(div/p/li)时收尾成一条消息。
fn parse_html(raw: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur_who = String::new();
    let mut cur_text = String::new();
    let b = raw.as_bytes();
    let mut i = 0usize;

    // 跳到目标闭合标签后的位置 (用于跳过 <span class="time">…</span> 等元数据容器)
    let skip_until_close = |raw: &str, from: usize, open_tag: &str| -> usize {
        let tag_name: String = open_tag
            .trim_start_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("span")
            .to_string();
        let close = format!("</{tag_name}>");
        match raw[from..].find(&close) {
            Some(p) => from + p + close.len(),
            None => raw.len(),
        }
    };

    while i < b.len() {
        if b[i] == b'<' {
            let close = raw[i + 1..].find('>').map(|p| i + 1 + p).unwrap_or(b.len());
            let tag = &raw[i + 1..close];
            let lower = tag.to_lowercase();
            if lower.starts_with('/') {
                // 闭合标签: div/p/li/br 等块级闭合 → 收尾当前消息
                let name: &str = lower.trim_start_matches('/').split_whitespace().next().unwrap_or("");
                if matches!(name, "div" | "p" | "li" | "tr") {
                    let t = cur_text.trim().to_string();
                    if !t.is_empty() && !cur_who.is_empty() {
                        out.push((cur_who.clone(), t));
                    }
                    cur_text.clear();
                    cur_who.clear();
                }
                i = close + 1;
            } else if is_sender_tag(&lower) {
                // 说话人标记: 取内部文本
                let inner_start = close + 1;
                let inner_end = raw[inner_start..].find('<').map(|p| inner_start + p).unwrap_or(b.len());
                let inner = raw[inner_start..inner_end].trim().to_string();
                if !inner.is_empty() {
                    // 新说话人: 先收尾上一条
                    let t = cur_text.trim().to_string();
                    if !t.is_empty() && !cur_who.is_empty() {
                        out.push((cur_who.clone(), t));
                    }
                    cur_who = inner;
                    cur_text.clear();
                    i = skip_until_close(raw, inner_end, tag);
                    continue;
                }
                i = close + 1;
            } else if is_meta_tag(&lower) {
                // 时间/日期/头像等元数据: 整个容器跳过
                i = skip_until_close(raw, close + 1, tag);
            } else {
                // 其它标签(内容容器/图片/链接): 继续扫描
                i = close + 1;
            }
        } else {
            // 文本节点
            let next = raw[i..].find('<').map(|p| i + p).unwrap_or(b.len());
            let seg = raw[i..next].trim();
            if !seg.is_empty() && !cur_who.is_empty() {
                let stripped = strip_tags(seg).trim().to_string();
                if !stripped.is_empty() && !looks_like_meta(&stripped) {
                    if !cur_text.is_empty() {
                        cur_text.push(' ');
                    }
                    cur_text.push_str(&stripped);
                }
            }
            i = next;
        }
    }
    // 收尾最后一条
    let t = cur_text.trim().to_string();
    if !t.is_empty() && !cur_who.is_empty() {
        out.push((cur_who.clone(), t));
    }
    out
}

/// 说话人标记: <span class="name"> / <div class="sender"> / <b> / <span class="user"> 等
fn is_sender_tag(lower: &str) -> bool {
    lower.contains("class=\"name\"") || lower.contains("class=\"sender\"")
        || lower.contains("class=\"user\"") || lower.contains("class=\"nickname\"")
        || lower.contains("class=\"talker\"") || lower == "b"
        || lower.starts_with("b ")
}

/// 元数据标签(其内容不是消息文本): 时间/日期/头像/系统
fn is_meta_tag(lower: &str) -> bool {
    lower.contains("class=\"time\"") || lower.contains("class=\"date\"")
        || lower.contains("class=\"avatar\"") || lower.contains("class=\"meta\"")
        || lower.contains("class=\"system\"")
}

/// 微信/QQ 电脑版导出的时间戳文本:
///   `2023-08-14 18:26:35 小美` / `2023-08-14 18:26:35 小美(12345678)`
///   下一行(或连续行)为内容。
fn parse_timed_text(raw: &str) -> Option<Vec<(String, String)>> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur_who: Option<String> = None;
    let mut cur_text = String::new();
    let mut found = false;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(who) = split_timed_sender(line) {
            // 上一条消息收尾
            if let Some(w) = cur_who.take() {
                let t = std::mem::take(&mut cur_text);
                if !t.is_empty() {
                    out.push((w, t));
                }
            }
            cur_who = Some(who);
            found = true;
        } else if cur_who.is_some() {
            // 内容行: 可能跨多行
            if !cur_text.is_empty() {
                cur_text.push('\n');
            }
            cur_text.push_str(line);
        }
    }
    if let Some(w) = cur_who {
        let t = std::mem::take(&mut cur_text);
        if !t.is_empty() {
            out.push((w, t));
        }
    }
    if found { Some(out) } else { None }
}

/// `YYYY-MM-DD HH:MM[:SS] 名字[(QQ号)]` → 名字
/// 仅当行首是「日期 时间」时识别, 避免把普通文本误判成时间戳格式。
fn split_timed_sender(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    let date = parts.next()?;
    let time = parts.next()?;
    // 日期: 至少含 4 位数字 + 分隔符; 时间: 含 ':' 的 HH:MM[:SS]
    if date.chars().filter(|c| c.is_ascii_digit()).count() < 4 {
        return None;
    }
    if !time.contains(':') {
        return None;
    }
    // 说话人: 名字 或 名字(12345678)
    let who_full = parts.next()?;
    let who = who_full.split('(').next().unwrap_or("").trim();
    if who.is_empty() { None } else { Some(who.to_string()) }
}

/// 纯文本: 每行 "名字: 内容" (容忍 "名字：内容" 全角冒号)
fn parse_simple_text(raw: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(pos) = line.find([':', '：']) {
            let who = line[..pos].trim().to_string();
            let text = line[pos + 1..].trim().to_string();
            if !text.is_empty() {
                out.push((who, text));
            }
        }
    }
    out
}

/// 去掉所有 HTML 标签。
fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// HTML 里时间/系统类标签行(不当作消息)。
fn looks_like_meta(line: &str) -> bool {
    line.contains("time") || line.contains("Date") || line.contains("date")
        || line.contains("system") || line.contains("notice")
        || line.contains("撤回") || line.contains("系统消息")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
小美: 你到家了嘛？
小美: 嗯嗯～
小明: 刚到
小美: 我今天买了一个超可爱的杯子！
小美: 给你看
小美: 哼哼😋
小美: 你猜多少钱
小美: 才20块！！
小美: 开心🥰
小美: 你下次来我家我给你用这个杯子喝咖啡
"#;

    #[test]
    fn extracts_speaker_style() {
        std::fs::write("sample_chat.txt", SAMPLE).unwrap();
        let p = extract_style("sample_chat.txt", "小美").unwrap();
        std::fs::remove_file("sample_chat.txt").ok();
        assert_eq!(p.speaker, "小美");
        assert!(p.msg_count >= 7);
        assert!(p.emoji_count > 0);
        assert!(!p.samples.is_empty());
        let prompt = style_prompt(&p);
        assert!(prompt.contains("小美"));
        assert!(prompt.contains("口头禅") || prompt.contains("说话风格"));
    }

    #[test]
    fn parses_wechat_timed_export() {
        // 微信电脑版「导出聊天记录」为 txt 的格式:
        //   时间戳行 + 内容行(可能跨行)
        let raw = "2023-08-14 18:26:35 小美\n你到家了嘛？\n2023-08-14 18:26:40 小明\n刚到，你呢\n2023-08-14 18:27:01 小美\n我今天买了一个超可爱的杯子！\n2023-08-14 18:27:30 小美\n你下次来我家\n我给你用这个杯子喝咖啡";
        let msgs = parse_messages(raw).unwrap();
        assert!(msgs.iter().any(|(w, t)| w == "小美" && t.contains("到家了")));
        assert!(msgs.iter().any(|(w, _)| w == "小明"));
        // 跨行内容合并
        let last = msgs.last().unwrap();
        assert_eq!(last.0, "小美");
        assert!(last.1.contains("下次来我家") && last.1.contains("喝咖啡"));
    }

    #[test]
    fn parses_qq_timed_export_with_qqnum() {
        // QQ 电脑版导出: 名字后带 QQ 号
        let raw = "2023-08-14 18:26:35 小美(12345678)\n你到家了嘛？\n2023-08-14 18:26:40 小明(87654321)\n刚到";
        let msgs = parse_messages(raw).unwrap();
        assert!(msgs.iter().any(|(w, _)| w == "小美"));
        assert!(msgs.iter().any(|(w, _)| w == "小明"));
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn parses_wechat_html_export() {
        let raw = r#"<html><body>
        <div class="msg"><span class="name">小美</span> <span class="time">18:26</span> 你到家了嘛？</div>
        <div class="msg"><span class="name">小明</span> 刚到</div>
        </body></html>"#;
        let msgs = parse_messages(raw).unwrap();
        assert!(msgs.iter().any(|(w, t)| w == "小美" && t.contains("到家了")));
        assert!(msgs.iter().any(|(w, t)| w == "小明" && t == "刚到"));
    }

    #[test]
    fn parses_qq_html_with_bold() {
        let raw = r#"<p><b>小美</b> 你到家了嘛？</p>
        <p><b>小明</b> 刚到</p>"#;
        let msgs = parse_messages(raw).unwrap();
        assert_eq!(msgs.len(), 2);
        assert!(msgs.iter().any(|(w, t)| w == "小美" && t.contains("到家了")));
    }

    #[test]
    fn parses_json_with_chatlog_key() {
        let raw = r#"{"chatLog":[{"user":"小美","message":"你到家了嘛？"},{"user":"小明","message":"刚到"}]}"#;
        let msgs = parse_messages(raw).unwrap();
        assert_eq!(msgs.len(), 2);
        assert!(msgs.iter().any(|(w, _)| w == "小美"));
    }

    #[test]
    fn parses_chatlog_tool_api() {
        // sjzar/chatlog 工具 /api/v1/chatlog 返回格式(私聊 + 群聊)
        let raw = r#"{"length":2,"messages":[
            {"time":"2025-08-27T00:00:00+08:00","talker":"wxid_123","talkerName":"小美","isSelf":false,"type":1,"content":"你到家了嘛？"},
            {"time":"2025-08-27T00:00:05+08:00","talker":"群名","talkerName":"","isChatRoom":true,"isSelf":false,"type":1,"content":"@小明 你到家了嘛？","sender":"小美"},
            {"time":"2025-08-27T00:00:06+08:00","talker":"wxid_123","talkerName":"小美","isSelf":false,"type":3,"content":"[图片]"}
        ]}"#;
        let msgs = parse_messages(raw).unwrap();
        // 私聊: 用 talkerName
        assert!(msgs.iter().any(|(w, t)| w == "小美" && t.contains("到家了")));
        // 群聊: 用 sender
        assert!(msgs.iter().any(|(w, t)| w == "小美" && t.contains("@小明")));
        // type=3(图片) 应被过滤
        assert!(!msgs.iter().any(|(_, t)| t.contains("[图片]")));
    }

    #[test]
    fn extracts_style_from_chatlog_api_messages() {
        // 模拟 chatlog API 返回 → extract_style_from_messages 全链路
        let raw = r#"{"length":6,"messages":[
            {"talkerName":"小美","type":1,"content":"你到家了嘛？"},
            {"talkerName":"小美","type":1,"content":"我今天买了一个超可爱的杯子！"},
            {"talkerName":"小美","type":1,"content":"才20块！！"},
            {"talkerName":"小美","type":1,"content":"你下次来我家我给你用这个杯子喝咖啡"},
            {"talkerName":"小明","type":1,"content":"刚到"},
            {"talkerName":"小美","type":3,"content":"[图片]"}
        ]}"#;
        let msgs = parse_messages(raw).unwrap();
        let p = extract_style_from_messages(&msgs, "小美").unwrap();
        assert_eq!(p.speaker, "小美");
        assert!(p.msg_count >= 4); // 图片(type=3)不计入
        let prompt = style_prompt(&p);
        assert!(prompt.contains("小美"));
        assert!(!prompt.is_empty());
    }
}
