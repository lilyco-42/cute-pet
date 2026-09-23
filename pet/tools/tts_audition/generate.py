#!/usr/bin/env python3
"""用仓库内已有的 aishell3 模型批量合成「每个 sid 的试听样本」。

动机(lyco O5 缩方案): 现用模型自带 174 个 speaker, 挑一个贴近丛雨气质的 sid
零包体成本。但选型必须靠人耳 —— 所以本脚本把 174 个音色一次性合成出来,
连编号一起念, 便于盲听扫描(单文件 all-sids.wav)与逐条试听(HTML)。

变量控制(信条 5): 模型文件 = APK 内同一份 dist-shell/assets/...; sherpa-onnx
1.13.8 = 与 Android AAR 同版本; noise_scale/noise_scale_w/length_scale =
TtsEngine.java 里的 0.667/0.8/1.0; num_threads=2 同 TtsEngine。
"""
import os
import sys
import wave

import numpy as np
import sherpa_onnx

# 路径: 默认按脚本位置推导, 可用环境变量覆盖(别在仓库里写死个人绝对路径)
HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))  # pet/tools/tts_audition -> 仓库根
MODEL_DIR = os.environ.get("PET_TTS_MODEL_DIR") or os.path.join(
    REPO, "dist-shell", "assets", "pet", "tts", "vits-icefall-zh-aishell3"
)
OUT_DIR = os.environ.get("PET_SID_OUT") or os.path.abspath(
    os.path.join(REPO, "..", "cute-pet-sid-audition")
)
SENTENCE = "你好，我是丛雨。"          # 试听句(与角色人设一致)
GAP_SEC = 0.45                          # 拼接时间隔


def build_tts():
    cfg = sherpa_onnx.OfflineTtsConfig(
        model=sherpa_onnx.OfflineTtsModelConfig(
            vits=sherpa_onnx.OfflineTtsVitsModelConfig(
                model=os.path.join(MODEL_DIR, "model.onnx"),
                lexicon=os.path.join(MODEL_DIR, "lexicon.txt"),
                tokens=os.path.join(MODEL_DIR, "tokens.txt"),
                data_dir="",
                dict_dir="",
                noise_scale=0.667,
                noise_scale_w=0.8,
                length_scale=1.0,
            ),
            num_threads=2,
            debug=False,
            provider="cpu",
        ),
        rule_fsts=os.path.join(MODEL_DIR, "date.fst") + "," + os.path.join(MODEL_DIR, "number.fst"),
        max_num_sentences=1,
    )
    if not cfg.validate():
        sys.exit("配置无效: model 目录是否齐全?")
    return sherpa_onnx.OfflineTts(cfg)


def write_wav(path, samples, sample_rate):
    pcm = np.clip(samples * 32767.0, -32768, 32767).astype(np.int16)
    with wave.open(path, "wb") as fh:
        fh.setnchannels(1)
        fh.setsampwidth(2)
        fh.setframerate(sample_rate)
        fh.writeframes(pcm.tobytes())


def main():
    tts = build_tts()
    n = tts.num_speakers
    sr = tts.sample_rate
    print(f"speakers={n}  sample_rate={sr}", flush=True)
    if not os.path.isdir(OUT_DIR):
        os.makedirs(OUT_DIR)

    montage = [np.zeros(int(sr * 0.3), dtype=np.float32)]
    gap = np.zeros(int(sr * GAP_SEC), dtype=np.float32)
    ok = 0
    fail = []
    for sid in range(n):
        text = f"{sid}，{SENTENCE}"       # 先念编号, 再念台词(便于盲听定位)
        try:
            audio = tts.generate(text, sid=sid, speed=1.0)
        except Exception as exc:  # noqa: BLE001 - 单个 sid 失败不该中断整批
            fail.append((sid, repr(exc)))
            continue
        s = np.asarray(audio.samples, dtype=np.float32)
        if s.size == 0:
            fail.append((sid, "空音频"))
            continue
        write_wav(os.path.join(OUT_DIR, f"sid_{sid:03d}.wav"), s, audio.sample_rate)
        montage.append(s)
        montage.append(gap)
        ok += 1
        if (sid + 1) % 20 == 0:
            print(f"  ... {sid + 1}/{n}", flush=True)

    write_wav(os.path.join(OUT_DIR, "all-sids.wav"), np.concatenate(montage), sr)
    print(f"完成: {ok}/{n} 个 sid", flush=True)
    if fail:
        print("失败:", fail[:10], flush=True)


if __name__ == "__main__":
    main()
