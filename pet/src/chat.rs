//! 聊天/语气层: Murasame persona + LLM 调用(env 门控) + 语料检索兜底。
//!
//! 架构:
//!   chat::Persona(风格样本) → respond(input)
//!     ├─ LLM 路径: 配置 PET_LLM_BASE_URL/PET_LLM_API_KEY/PET_LLM_MODEL 时调用
//!     └─ 兜底路径: 从丛雨台词语料中检索/随机回应(离线可用)

use serde_json::json;

/// 回复语言模式: 中文 / 日文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    Jp,
}

impl Lang {
    pub fn label(self) -> &'static str {
        match self {
            Lang::Zh => "中文",
            Lang::Jp => "日本語",
        }
    }
    /// 追加到 LLM system prompt 的语言约束。
    pub fn prompt_rule(self) -> &'static str {
        match self {
            Lang::Zh => "\n【语言要求】请务必只用简体中文回复, 不要混入日语/假名。",
            Lang::Jp => "\n【言語要件】必ず日本語で返答してください。",
        }
    }
}

/// 过滤中文语料里的"垃圾日语": 含假名(平/片假名, 含长音记号 ー)的台词直接丢弃,
/// 保证中文模式回复纯中文。日文模式不做过滤。
pub fn filter_zh_corpus(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(d) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let text = d["text"].as_str().unwrap_or("");
            if has_kana(text) {
                continue;
            }
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    out
}

/// 一个可说话的角色: 名字 + 风格系统提示 + 语料 + 语音码。
pub struct Persona {
    pub name: String,
    pub system_prompt: String,
    /// (台词, 语音引用) 语料, 用于兜底回应。
    pub corpus: Vec<(String, String)>,
    /// 语料字符 IDF(逆文档频率): 罕见字命中权重高, 虚词(了/吗/你)自动降权。
    char_idf: std::collections::HashMap<char, f32>,
    /// 语音码, 对应 voice.xp3 的 murXXX_YYY。留空则无语音。
    pub voice_code: String,
}

impl Persona {
    /// 追加语言约束到 system prompt(构建后调用)。
    pub fn set_language(&mut self, lang: Lang) {
        self.system_prompt.push_str(lang.prompt_rule());
    }
    /// 从 transcript.jsonl 构建丛雨 persona。
    /// 语料格式: {"who":"ムラサメ","voices":["mur001_001"],"text":"..."}
    pub fn murasame_from_corpus(transcript_path: &str) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(transcript_path)?;
        Self::murasame_from_corpus_content(&raw)
    }

    /// 从语料内容字符串构建(跨平台: Android/WASM 用 load_file 取内容再传入)。
    pub fn murasame_from_corpus_content(raw: &str) -> anyhow::Result<Self> {
        let mut corpus = Vec::new();
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(d) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let text = d["text"].as_str().unwrap_or("").trim().to_string();
            let voice = d["voices"]
                .as_array()
                .and_then(|v| v.first())
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !text.is_empty() && !voice.is_empty() {
                corpus.push((text, voice));
            }
        }
        // 角色切换: PET_CHARACTER=飛花|兰雀|丛雨(murasame), 或 PET_PERSONA_FILE 直接指定 persona 文件
        let character = std::env::var("PET_CHARACTER").unwrap_or_default();
        let persona_from_file = |p: &str| std::fs::read_to_string(p).ok();
        let mut system_prompt = if !character.is_empty()
            && character != "murasame"
            && character != "丛雨"
        {
            // 依次尝试: ../characters/(cute_box 根) 与 assets/characters/(pet 内)
            let candidates = [
                format!("../characters/{character}_persona.txt"),
                format!("assets/characters/{character}_persona.txt"),
            ];
            candidates
                .iter()
                .find_map(|c| persona_from_file(c))
                .unwrap_or_else(|| include_str!("../assets/murasame_persona.txt").to_string())
        } else if let Ok(p) = std::env::var("PET_PERSONA_FILE") {
            persona_from_file(&p).unwrap_or_else(|| include_str!("../assets/murasame_persona.txt").to_string())
        } else {
            include_str!("../assets/murasame_persona.txt").to_string()
        };
        // 可选: 导入「喜欢的人」聊天记录学习语气。
        //   本地文件:  PET_STYLE_LOG=/path/chat.txt + PET_STYLE_SPEAKER=名字
        //   chatlog 服务: PET_CHATLOG_URL=http://127.0.0.1:5030 + PET_STYLE_SPEAKER=名字
        let speaker = std::env::var("PET_STYLE_SPEAKER").unwrap_or_else(|_| "对方".to_string());
        if let Ok(base) = std::env::var("PET_CHATLOG_URL") {
            #[cfg(not(target_arch = "wasm32"))]
            match crate::chatlog::extract_style_from_chatlog(&base, &speaker) {
                Ok(profile) => {
                    system_prompt.push_str(&crate::chatlog::style_prompt(&profile));
                    eprintln!("[chat] 已从 chatlog API 学习语气: {speaker}({} 条)", profile.msg_count);
                }
                Err(e) => eprintln!("[chat] chatlog 语气学习失败: {e}"),
            }
            #[cfg(target_arch = "wasm32")]
            let _ = (base, speaker.clone());
        } else if let Ok(log) = std::env::var("PET_STYLE_LOG") {
            match crate::chatlog::extract_style(&log, &speaker) {
                Ok(profile) => {
                    system_prompt.push_str(&crate::chatlog::style_prompt(&profile));
                    eprintln!("[chat] 已学习聊天语气: {speaker}({} 条)", profile.msg_count);
                }
                Err(e) => eprintln!("[chat] 语气学习失败: {e}"),
            }
        }
        let name = if character.is_empty() || character == "murasame" || character == "丛雨" {
            "ムラサメ".to_string()
        } else {
            character.clone()
        };
        // 统计语料字符 IDF: 每个字出现在多少条语料中(去标点/空白)
        let norm = |s: &str| -> String {
            s.chars()
                .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation() && !matches!(c, '「' | '」' | '？' | '。' | '…' | '、' | '！' | '　'))
                .collect()
        };
        let mut doc_freq: std::collections::HashMap<char, usize> = std::collections::HashMap::new();
        for (text, _) in &corpus {
            let t = norm(text);
            let mut seen = std::collections::HashSet::new();
            for c in t.chars() {
                if seen.insert(c) {
                    *doc_freq.entry(c).or_insert(0) += 1;
                }
            }
        }
        let n = corpus.len().max(1) as f32;
        let char_idf: std::collections::HashMap<char, f32> = doc_freq
            .iter()
            .map(|(&c, &df)| (c, (n / (df as f32 + 1.0)).ln() + 1.0))
            .collect();
        Ok(Self {
            name,
            system_prompt,
            corpus,
            char_idf,
            voice_code: "mur".to_string(),
        })
    }

    /// 用 LLM 生成回复(OpenAI 兼容 chat/completions)。
    /// 未配置密钥/请求失败 → Err, 调用方回退到语料。仅非 WASM(需要网络)。
    #[cfg(not(target_arch = "wasm32"))]
    pub fn respond_llm(
        &self,
        history: &[(String, String)],
        input: &str,
    ) -> anyhow::Result<String> {
        let base = std::env::var("PET_LLM_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".into());
        let key = std::env::var("PET_LLM_API_KEY").map_err(|_| anyhow::anyhow!("PET_LLM_API_KEY 未配置"))?;
        let model = std::env::var("PET_LLM_MODEL").unwrap_or_else(|_| "deepseek-chat".into());

        let mut messages = vec![json!({"role": "system", "content": self.system_prompt})];
        for (who, text) in history {
            messages.push(json!({"role": if *who == self.name { "assistant" } else { "user" }, "content": text}));
        }
        messages.push(json!({"role": "user", "content": input}));

        let url = format!("{}/v1/chat/completions", base.trim_end_matches('/'));
        let body = json!({"model": model, "messages": messages, "temperature": 0.9});
        let mut resp = ureq::post(&url)
            .header("Authorization", &format!("Bearer {}", key))
            .header("Content-Type", "application/json")
            .send_json(&body)?;
        let raw = resp.body_mut().read_to_string()?;
        let value: serde_json::Value = serde_json::from_str(&raw)?;
        let text = value["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("LLM 响应无 content"))?
            .trim()
            .to_string();
        Ok(text)
    }

    /// WASM 桩: 无网络, 返回 Err 触发语料兜底。
    #[cfg(target_arch = "wasm32")]
    pub fn respond_llm(
        &self,
        _history: &[(String, String)],
        _input: &str,
    ) -> anyhow::Result<String> {
        anyhow::bail!("WASM 无网络 LLM")
    }

    /// 兜底: 从语料里检索回应。用 IDF 加权的连续子串重叠打分
    /// (罕见字命中高分, 虚词自动降权, 避免哈希随机的答非所问)。无语料返回 None。
    pub fn respond_corpus(&self, input: &str) -> Option<(String, String)> {
        let n = self.corpus.len();
        if n == 0 {
            return None;
        }
        // 归一化: 去标点/空白/引号
        let norm = |s: &str| -> String {
            s.chars()
                .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation() && !matches!(c, '「' | '」' | '？' | '。' | '…' | '、' | '！' | '　'))
                .collect()
        };
        let norm_text = norm(input);
        if norm_text.is_empty() {
            return Some(self.corpus[0].clone());
        }
        let chars: Vec<char> = norm_text.chars().collect();
        // 输入里每个字的 IDF(未登录字给中值权重)
        let idf = |c: char| self.char_idf.get(&c).copied().unwrap_or(1.0);
        // 生成输入的 2-4 字子串集合(去重)
        let mut subs: Vec<String> = Vec::new();
        for w in 2..=chars.len().min(4) {
            for win in chars.windows(w) {
                let s: String = win.iter().collect();
                if !subs.contains(&s) {
                    subs.push(s);
                }
            }
        }
        let mut best: Option<(f32, usize)> = None;
        for (i, (text, _)) in self.corpus.iter().enumerate() {
            let t = norm(text);
            if t.is_empty() {
                continue;
            }
            let mut score = 0.0f32;
            for sub in &subs {
                if t.contains(sub.as_str()) {
                    // 子串得分 = 子串长度 × 各字 IDF 之和(罕见字重叠 → 强信号)
                    score += sub.chars().map(idf).sum::<f32>() * sub.chars().count() as f32;
                }
            }
            match best {
                Some((bs, _)) if score > bs => best = Some((score, i)),
                Some(_) => {}
                None => best = Some((score, i)),
            }
        }
        let (score, idx) = best.unwrap();
        // 命中太低视为无相关台词, 回退第一条通用台词避免胡言乱语
        let min_score = chars.iter().map(|&c| idf(c) * 2.0).sum::<f32>() * 0.35;
        let idx = if score >= min_score { idx } else { 0 };
        let (text, voice) = &self.corpus[idx];
        Some((text.clone(), voice.clone()))
    }
}

/// 预置问答匹配: 否定前缀排除 + 更长关键词优先 + 忽略标点(关键词带"?"可匹配不带"?"的输入)。
/// 供 main.rs 预置问答使用(未命中返回 None, 走 persona 语料)。
pub fn preset_match(kws: &[String], input: &str) -> Option<usize> {
    let negated = ["不", "没", "别", "讨厌", "不要", "不想", "不会", "没有", "才不"]
        .iter()
        .any(|neg| input.trim_start().starts_with(neg));
    if negated {
        return None;
    }
    let strip = |s: &str| -> String {
        s.chars()
            .filter(|c| !c.is_ascii_punctuation() && *c != '？' && *c != '。' && *c != '，' && *c != '！')
            .collect()
    };
    let inp = strip(input);
    kws.iter()
        .enumerate()
        .filter(|(_, kw)| inp.contains(&strip(kw)))
        .max_by_key(|(_, kw)| strip(kw).chars().count())
        .map(|(i, _)| i)
}

/// 用远程 GPT-SoVITS TTS 服务合成文本为 wav 字节(丛雨克隆音色)。
/// 服务地址经 `PET_TTS_URL` 环境变量配置; 未配置/不可用时返回 Err,
/// 调用方回退到端侧方案(Web Speech API / 内嵌兜底语音)。仅非 WASM。
#[cfg(not(target_arch = "wasm32"))]
pub fn synthesize_remote(text: &str) -> anyhow::Result<Vec<u8>> {
    let base = std::env::var("PET_TTS_URL").map_err(|_| anyhow::anyhow!("PET_TTS_URL 未配置(已停用默认云端 TTS)"))?;
    let encoded = urlencode(text);
    let url = format!("{}/tts?text={}", base.trim_end_matches('/'), encoded);
    let mut resp = ureq::get(&url).call()?;
    let buf = resp.body_mut().read_to_vec()?;
    Ok(buf)
}

/// WASM 桩: 无网络 TTS。
#[cfg(target_arch = "wasm32")]
pub fn synthesize_remote(_text: &str) -> anyhow::Result<Vec<u8>> {
    anyhow::bail!("WASM 无网络 TTS")
}

/// 简单 UTF-8 百分号编码(保留 ASCII 字母数字, 编码其余字节)。
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 判断文本是否含日文假名(片假名 30A0-30FF / 平假名 3040-309F)。
/// 含假名视为日文台词, 需要中译; 纯汉字/中文不翻译。
pub fn has_kana(text: &str) -> bool {
    text.chars().any(|c| matches!(c, '\u{3040}'..='\u{30ff}'))
}

/// 把日文台词翻译成中文(带丛雨口吻)。配置了 PET_LLM_API_KEY 才生效;
/// 未配置或请求失败返回 None(调用方回退显示原文)。
/// 仅非 WASM(需要网络)。
#[cfg(not(target_arch = "wasm32"))]
pub fn translate_to_chinese(jp_text: &str) -> Option<String> {
    if !has_kana(jp_text) {
        return None;
    }
    let key = std::env::var("PET_LLM_API_KEY").ok()?;
    let base = std::env::var("PET_LLM_BASE_URL").unwrap_or_else(|_| "https://api.deepseek.com".into());
    let model = std::env::var("PET_LLM_MODEL").unwrap_or_else(|_| "deepseek-chat".into());
    let system = "你是汉化翻译。把用户的日文台词翻译成自然的中文, 保留说话人的语气和口癖(如「吾辈」), 只输出译文, 不要解释。";
    let url = format!("{}/v1/chat/completions", base.trim_end_matches('/'));
    let body = json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": jp_text}
        ],
        "temperature": 0.3,
    });
    let mut resp = ureq::post(&url)
        .header("Authorization", &format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .send_json(&body)
        .ok()?;
    let raw = resp.body_mut().read_to_string().ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let text = value["choices"][0]["message"]["content"].as_str()?.trim().to_string();
    if text.is_empty() || text == jp_text {
        None
    } else {
        Some(text)
    }
}

/// WASM 桩: 无网络翻译。
#[cfg(target_arch = "wasm32")]
pub fn translate_to_chinese(_jp_text: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_loads() {
        let p = Persona::murasame_from_corpus("assets/murasame_corpus.jsonl").unwrap();
        assert!(!p.corpus.is_empty());
        let (text, voice) = p.respond_corpus("こんにちは").unwrap();
        assert!(!text.is_empty());
        assert!(voice.starts_with("mur"));
    }

    #[test]
    fn detects_japanese_kana() {
        assert!(has_kana("「ふむ。お主が、吾輩のご主人か？」"));
        assert!(has_kana("こんにちは"));
        assert!(!has_kana("你到家了嘛？"));
        assert!(!has_kana("吾辈可是活了数百年的刀灵"));
    }

    #[test]
    fn retrieval_semantic_relevance() {
        let p = Persona::murasame_from_corpus("assets/murasame_corpus_zh.jsonl").unwrap();
        let cases = [
            "我喜欢你",
            "你好呀",
            "吃饭了吗",
            "今天天气怎么样",
            "你叫什么名字",
            "我回来了",
            "你最近在做什么",
            "不要走",
        ];
        for input in cases {
            let (text, _voice) = p.respond_corpus(input).unwrap();
            println!("输入: {input}\n  → {text}");
        }
    }

    #[test]
    fn preset_match_no_false_positive() {
        let kws: Vec<String> = ["想我了吗？", "吃饭了吗？", "喜欢什么？", "在吗？"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // 否定前缀不误中
        assert_eq!(preset_match(&kws, "我不想你"), None);
        assert_eq!(preset_match(&kws, "我还没吃饭"), None);
        // 正常命中(输入可不带问号)
        assert_eq!(preset_match(&kws, "想我了吗"), Some(0));
        assert_eq!(preset_match(&kws, "你今天吃饭了吗？"), Some(1));
        // 更长关键词优先
        assert_eq!(preset_match(&kws, "你喜欢什么？"), Some(2));
    }

    /// 集成测试: 连 CloudStudio 云端 lyco_chat(需已部署, 见 docs/lyco-chat-cloud-deploy.md)。
    /// 运行: cargo test llm_cloud -- --ignored
    #[test]
    #[ignore]
    fn llm_cloud_integration() {
        std::env::set_var(
            "PET_LLM_BASE_URL",
            "https://04e7e16c9cac40dda427befd85ead378--8080.ap-shanghai2.cloudstudio.club",
        );
        std::env::set_var("PET_LLM_API_KEY", "cloudstudio");
        std::env::set_var("PET_LLM_MODEL", "lyco");
        let p = Persona::murasame_from_corpus("assets/murasame_corpus_zh.jsonl").unwrap();
        let reply = p.respond_llm(&[], "你好呀，丛雨").unwrap();
        assert!(!reply.is_empty());
        eprintln!("[云端AI] {reply}");
    }
}
