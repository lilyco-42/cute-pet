use std::fs;
use std::path::PathBuf;
use yuzu_xp3::Xp3Archive;
use yuzu_scn::Line;

/// 从中文版 patch.xp3 提取丛雨(ムラサメ)中译台词语料 → JSONL
/// 输出结构对齐现有 murasame_corpus.jsonl: {"who","text","voices"}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let xp3_path = args.get(1).cloned().unwrap_or_else(|| {
        "D:/Code/cute_box/assets/xp3_apk/assets/file/patch.xp3".to_string()
    });
    let out_jsonl = args.get(2).cloned().unwrap_or_else(|| {
        "C:/Users/liuqi/AppData/Local/Temp/opencode/zh_murasame_corpus.jsonl".to_string()
    });

    let arc = Xp3Archive::open_file(&xp3_path).expect("打开 XP3 失败");
    println!("[+] 文件数: {}", arc.len());

    let names: Vec<String> = arc.file_names().map(|s| s.to_string()).collect();
    let mut corpus: Vec<(String, String, Vec<String>)> = Vec::new(); // (who, cn_text, voices)

    for name in &names {
        if !name.ends_with(".ks.scn") {
            continue;
        }
        let data = match arc.read(name) {
            Ok(Some(d)) => d,
            _ => continue,
        };
        match yuzu_scn::parse(&data) {
            Ok(scn) => {
                for scene in &scn.scenes {
                    // lines 引用了 texts 索引
                    for text in &scene.texts {
                        // 说话人: character 或 dialogue.name
                        let who = text
                            .character
                            .clone()
                            .or_else(|| {
                                text.dialogues.first().and_then(|d| d.name.clone())
                            })
                            .unwrap_or_default();
                        // 中文台词
                        for d in &text.dialogues {
                            let text_str: Option<String> = match &d.content {
                                yuzu_scn::Content::Lang(map) => map.get("cn").map(|s| s.to_string()),
                                yuzu_scn::Content::Plain(s) => Some(s.clone()),
                            };
                            if let Some(t) = text_str {
                                let t = t.trim();
                                if !t.is_empty() && t != "…" && t != "…。" {
                                    let voices: Vec<String> = text
                                        .voices
                                        .iter()
                                        .filter_map(|v| v.voice.clone())
                                        .collect();
                                    corpus.push((who.clone(), t.to_string(), voices));
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }

    println!("[+] 提取台词: {}", corpus.len());

    // 只保留有语音的(对齐现有语料, 语音码开头 mur)
    let mut out = String::new();
    let mut n = 0;
    for (who, text, voices) in &corpus {
        if voices.is_empty() {
            continue;
        }
        let v: Vec<&str> = voices.iter().map(|s| s.as_str()).collect();
        let line = format!(
            "{{\"who\":{},\"text\":{},\"voices\":{:?}}}\n",
            serde_json::to_string(who).unwrap_or_default(),
            serde_json::to_string(text).unwrap_or_default(),
            v
        );
        out.push_str(&line);
        n += 1;
    }
    fs::write(&out_jsonl, out).expect("写输出失败");
    println!("[+] 写入 {} 条语音台词 → {out_jsonl}", n);
    println!("[+] 样例:");
    for line in fs::read_to_string(&out_jsonl).unwrap().lines().take(3) {
        println!("  {line}");
    }
}