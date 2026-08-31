# 丛雨语音克隆 — GPT-SoVITS 训练指南

目标: 用丛雨的真实语音训练 GPT-SoVITS, 让桌宠说出任意新台词时都像丛雨的声音。

## 数据(已就绪)
- `pet/assets/sovits/murasame.list` — 300 条「wav路径|日文台词|说话人」对齐对
- `pet/assets/sovits/wavs/*.wav` — 16kHz 单声道 WAV(300 条, 已用 ffmpeg 从 voice.xp3 的 M4A 转换)
- 全量语料(4405 条)在 `../assets/pet/murasame/corpus/`(可换大语料提升质量)

## 安装(需要 NVIDIA GPU + CUDA)
```bash
# 1. 克隆 GPT-SoVITS
git clone https://github.com/RVC-Boss/GPT-SoVITS.git
cd GPT-SoVITS
# 2. 建议用 uv/conda 建环境
uv venv .venv && source .venv/bin/activate   # 或 conda create -n gpt-sovits python=3.10
pip install -r requirements.txt
```

## 训练(微调 丛雨 音色)
```bash
cd GPT-SoVITS
# 1. 数据预处理: 生成语义 token + f0
python prepare_datasets.py \
  --text  <本仓库>/pet/assets/sovits/murasame.list \
  --save  data/murasame \
  --coarse-clip data/pretrained_models/chinese-hubert-base/ \
  --pretrained_sovits pretrained_models/s2G2333k.pth \
  --pretrained_gpt pretrained_models/s1bert25hz-2kh-longer-epoch=60e-step=5024.ckpt

# 2. 微调 SoVITS(音色, 建议 100-200 epoch, 1-2 小时)
python train.py -c configs/s1longer-v2.yaml \
  -m data/murasame -t SoVITS -s <speaker_id> \
  --pretrained_s2 pretrained_models/s2G2333k.pth \
  --pretrained_s1 pretrained_models/s1bert25hz-2kh-longer-epoch=60e-step=5024.ckpt

# 3. 推理(合成任意台词)
python inference_webui.py  # 打开 WebUI, 选微调模型 + 参考音频, 输入台词
```

## 备选: 零样本(zero-shot, 不训练)
GPT-SoVITS 支持零样本克隆: 只需 5-10 秒参考音频。取一段丛雨语音:
```bash
# 从语料挑一段 ~5s 的干净台词
ffmpeg -i <murasame_voice>.ogg -ar 16000 -ac 1 ref.wav
```
在推理 WebUI 里: 选 `pretrained_models/s2G2333k.pth` + 参考音频 ref.wav + 输入任意台词 → 直接出丛雨声线的合成音频。质量略低但立即可用。

## 接入桌宠
合成出的 wav 放进 `pet/assets/voice/`, 用现有播放逻辑即可让桌宠说任意话。
后续可把 TTS 合成接进 `chat.rs` 的 LLM 回复路径: LLM 出文本 → TTS 出丛雨语音 → 播放。
