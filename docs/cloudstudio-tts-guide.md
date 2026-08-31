# CloudStudio GPT-SoVITS 丛雨语音合成指南

> 目标: 让任意 AI Agent 通过 CloudStudio 云 GPU + Jupyter, 用 GPT-SoVITS 合成「丛雨」克隆语音(日语/中文),
> 并验证音色 + 内容是否正确。本文来自实战(2026-08-31), 完整记录了**正确的模型组合**与**踩过的坑**。
>
> 认证/JPS 访问流程见 [cloudstudio-access.md](cloudstudio-access.md); 本文只讲**合成**。

---

## 0. 一句话核心经验(最重要)

**合成「丛雨」语音, GPT(语义模型)必须用「通用大模型」, 不能用「丛雨微调版」!**

- GPT 模型决定「说什么内容」(语义 token 预测)
- SoVITS(音色)模型决定「用什么声音说」

我之前一直用 **丛雨微调版 GPT**(`murasame_s1.ckpt`, 只微调 300 条语料)。
它**过拟合严重**——即使让模型「照读原声同一句」, 合出来的也是完全不同内容
(ASR 识别为乱码; 而原声本身能被 large-v3 100% 识别)。

**改用通用 GPT(`lain-e15`)后, 同一目标句合成内容就几乎全对**:
```
目标: 主と一緒にいられて、吾輩は嬉しいぞ
合成ASR: "ウェシと一緒にいられて、ワダハイは嬉しいぞ。"   ← 内容几乎全对, 仅丛雨口音音素差异
```

> 之前 agent「中文能出、日文乱」的真因: 中文靠通用 GPT 语义凑合能读, 日文被过拟合的丛雨 GPT 毁了。
> 用通用 GPT 就两头都通。

---

## 1. 正确模型组合(已验证可出正确内容)

| 角色 | 路径 | 说明 |
|---|---|---|
| **GPT(语义)** | `/workspace/GPT_weights_v2Pro/lain-e15.ckpt` | **通用大模型**, 语义泛化好(关键!) |
| **SoVITS(音色)** | `/workspace/GPT_SoVITS/data/murasame/logs_s2_v2/G_233333333333_infer.pth` | 丛雨微调 v2, 推理版 |
| 参考音频 | `/workspace/mur_refs/mur309_017.wav` | 「吾輩は嬉しいぞ。これからもよろしく頼む」, 与目标句尾最贴近 |

备选通用 GPT(`/workspace/GPT_weights_v2Pro/`): `lain-e5/e10/e15`, `Dan-e5/e10/e15`, `Dang2-*`。
丛雨 SoVITS 也可用最新训练版 `.../G_233333333333.pth`(含 optimizer, 合成会用)。

### 为什么通用 GPT + 丛雨 SoVITS 是对的组合?
GPT-SoVITS 是「音素 → 语义token → 音色」两段式:
- **GPT**: 文本音素 → 语义 token(决定内容/语调语速)
- **SoVITS**: 语义 token + 参考音色 → 波形(决定音色)

丛雨微调 GPT 只在 300 条语料上学的语义映射, 对**训练集之外的句子**泛化差 → 内容崩。
通用 GPT 在海量语料上训的语义, 泛化好 → 内容对。音色由丛雨 SoVITS 提供, 所以听感仍是丛雨。

---

## 2. 快速开始(5 步合成)

### 2.1 认证 + 拿 JPS/JWT
```bash
export CS_COOKIE='cloudstudio-session=<值>; cloudstudio-session-team=gh'
SPACE=<32位spaceKey>        # 从工作空间预览域名提取
node cs_auth.mjs $SPACE      # 打印 JPS + TOKEN(5分钟过期) + export 命令
```

### 2.2 准备参考音频(3~10 秒, 关键!)
GPT-SoVITS 要求参考音频 **3~10 秒**, 否则报错 `参考音频在3~10秒范围外`。
```bash
# 从丛雨原声 ogg 转 32k wav(ffmpeg)
ffmpeg -y -i assets/pet/murasame/corpus/voice/<voice>.ogg -ar 32000 -ac 1 ref.wav
node cs_upload.mjs ref.wav mur_refs/<voice>.wav   # 上传到云端
```
> 参考音频**越贴近目标句**(同句尾/同语气)合成越好。用 `grep` 语料找:
> 目标「吾輩は嬉しいぞ」→ 用 `mur309_017`(「吾輩は嬉しいぞ。…」)完美匹配。

### 2.3 写参考/目标文本
参考文本用参考音频的**原台词**; 目标是你要说的日语。
```bash
# 云端写文件(经 cs_exec.mjs)
echo '吾輩は嬉しいぞ。これからもよろしく頼む' > /workspace/GPT_SoVITS/ref_text.txt
echo '主と一緒にいられて、吾輩は嬉しいぞ'     > /workspace/GPT_SoVITS/target_text.txt
```

### 2.4 合成(inference_cli.py)
```bash
cd /workspace/GPT_SoVITS
MPLBACKEND=Agg PYTHONPATH="/workspace/GPT_SoVITS:/workspace/GPT_SoVITS/eres2net:/workspace" \
bert_path=/workspace/GPT_SoVITS/pretrained_models/chinese-roberta-wwm-ext-large \
cnhubert_base_path=/workspace/GPT_SoVITS/pretrained_models/chinese-hubert-base \
/root/miniconda/envs/GPTSoVits/bin/python inference_cli.py \
  --gpt_model /workspace/GPT_weights_v2Pro/lain-e15.ckpt \
  --sovits_model /workspace/GPT_SoVITS/data/murasame/logs_s2_v2/G_233333333333_infer.pth \
  --ref_audio /workspace/mur_refs/mur309_017.wav \
  --ref_text /workspace/GPT_SoVITS/ref_text.txt --ref_language 日文 \
  --target_text /workspace/GPT_SoVITS/target_text.txt --target_language 日文 \
  --output_path /workspace/GPT_SoVITS/mur_out
# 输出: /workspace/GPT_SoVITS/mur_out/output.wav
```

### 2.5 验证(下载 + ASR)
合成后**必须**用 ASR 验证内容对不对(否则可能合成出乱码还以为成功)。
```bash
node cs_download.mjs "GPT_SoVITS/mur_out/output.wav" murasame_jp.wav   # 注意相对路径, 不带前导 /
# 云端 ASR(用 large-v3 最准; small/medium 对丛雨音色会误判)
python - <<'PY'
from faster_whisper import WhisperModel
m=WhisperModel("large-v3",device="cpu",compute_type="int8",download_root="/workspace/whisper_models")
segs,info=m.transcribe("/workspace/GPT_SoVITS/mur_out/output.wav",language="ja",beam_size=5)
print("ASR:", " ".join(s.text.strip() for s in segs))
PY
```
> **判定标准**: 原声参考能被 large-v3 准确识别(已确认); 合成接近目标文本 = 成功。

---

## 3. 坑与排错(全部踩过)

| 症状 | 根因 | 解法 |
|---|---|---|
| **合成内容乱(连照读原声都对不上)** | GPT 用了丛雨微调版, 过拟合 | **改用通用 GPT lain-e15** |
| `ModuleNotFoundError: GPT_SoVITS` | cwd/PYTHONPATH 不对 | `cd /workspace` + `PYTHONPATH=/workspace` 且以 `GPT_SoVITS/...` 前缀调用 |
| `No module named 'x_transformers'` | v2 依赖缺失 | `pip install x_transformers` |
| `参考音频在3~10秒范围外` | 参考太短(<3s)或太长(>10s) | 挑 3~10s 的音频(用 ffprobe 查时长) |
| `inference_cli --ref_text` 传字符串报 FileNotFound | 参数名有误导: 实为**文件路径** | 写文本到文件, 传文件路径 |
| ASR small/medium 识别乱 | 小模型对动漫音色泛化差 | 用 **large-v3**; 并先测原声参考确认 ASR 准 |
| 合成日志大量 `enc_q unexpected_keys` | v2 模型可选编码器警告 | 无害, 可忽略(不影响内容) |
| `soundfile.LibsndfileError: System error` | `--output_path` 目录不存在 | 先 `mkdir -p` 输出目录 |
| `cn2an 缺失`(独立 import 时) | 需 GPTSoVits env | 用 `/root/miniconda/envs/GPTSoVits/bin/python` |

---

## 4. 推理引擎

环境有两套引擎(旧 `inference_webui.py` vs 新 `TTS_infer_pack/`)。
**用 `inference_cli.py`(它 import `inference_webui`)即可**, 与之前 agent 验证中文时一致。
`inference_webui.py` 的模型版本(判定 `_infer.pth` → v2)是自动的, 无需手动指定。

---

## 5. 已验证的参考音频(丛雨, 3~10s)

| voice | 时长 | 台词 | 适合目标 |
|---|---|---|---|
| `mur309_017` | 3.6s | 吾輩は嬉しいぞ。これからもよろしく頼む | 「…嬉しいぞ」结尾 |
| `mur317_111` | 4.4s | 優しい神様がいてくれて、吾輩は嬉しいぞ | 同上 |
| `mur309_220` | 3.5s | 嬉しいぞ……ご主人…… | 短句 |
| `mur304_059` | 4.3s | ご主人がそこまで言ってくれるのは、吾輩も嬉しい | 敬语长句 |
| `mur001_001` | 3.9s | ふむ。お主が、吾輩のご主人か？ | 通用开场 |

> 挑选方法: 从语料 JSONL 找目标句关键词, 取时长 3~10s 的 mur* 音频, 转 wav 上传两。

---

## 6. 完整数据(供扩充训练)

- 语料: `/workspace/GPT_SoVITS/murasame_data/murasame.list`(300 条当前训练用)
- 本地全量: `pet/assets/murasame_corpus.jsonl`(4400+ 条丛雨台词 + voice 映射), 音频在
  `assets/pet/murasame/corpus/voice/*.ogg`(4566 个)
- 音素/语义: `/workspace/GPT_SoVITS/data/murasame/2-name2text.txt`(音素), `6-name2semantic.tsv`(语义token)

> 若要提升泛化到更多句子, 应把训练数据从 300 条扩充到 4400 条(音频需先转 32k wav)。
> 但**推理侧用通用 GPT 已能出正确内容**, 扩充训练属可选优化。
