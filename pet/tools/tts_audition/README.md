# 音色试听工具（tts_audition）

壳内离线 TTS 用 `vits-icefall-zh-aishell3`（**174 个说话人**，`sid` 0..173）。
挑一个贴近丛雨气质的音色是**零包体成本**的收益，但选型必须靠耳朵 —— 这目录里的两个
脚本就是"程序初筛 + 人耳终选"的工具链：先把 174 个音色一次性合成出来（顺带算出基频，
把女声区排前面），人耳只需要听前 20 个。

## 用法

```bash
# 依赖（与 APK 内同一版本, 避免变量不一致）
pip install sherpa-onnx numpy        # 1.13.8 = Android AAR 版本

# 1) 批量合成: 每个 sid 念「<编号>，你好，我是丛雨。」
python generate.py                   # 产出 sid_000.wav ... sid_173.wav + all-sids.wav
python index.py                      # 算中位 F0, 生成可试听的 index.html
```

产物默认写在仓库**外**的 `../cute-pet-sid-audition/`（脚本按自身位置推导，不写死个人路径）。
可用环境变量覆盖：`PET_TTS_MODEL_DIR`（模型目录，默认 `dist-shell/assets/pet/tts/vits-icefall-zh-aishell3`）、
`PET_SID_OUT`（输出目录）。
`all-sids.wav` 是拼接版：每个音色先念编号再念台词，**可以一遍盲听扫描**。

## 选定后怎么生效：`tts_sid.txt`

不用重建 APK。在**用户素材目录**里放一个纯文本文件，内容就是编号：

```
/sdcard/Android/data/rust.cute_pet/files/assets/pet/tts_sid.txt   →  27
```

- 优先级：**素材目录文件 > SharedPreferences(`tts_native_sid`) > 0**
- 每次说话都会重新读 → 改完**即时生效**（不必重启服务，也不必 adb / root）
- 生效值会打日志：`CutePetTts: TTS sid = 27 (来源: 素材目录文件 …)`
- 想固化进版本：改 `OverlayService` 里 `getInt(PREF_TTS_SID, 0)` 的默认值并重新构建

## 变量控制（为什么这套数字可信）

模型文件直接取 **APK 内同一份** `dist-shell/assets/pet/tts/vits-icefall-zh-aishell3/`；
`sherpa-onnx` 版本与 Android AAR 相同（1.13.8）；`noise_scale=0.667 / noise_scale_w=0.8 /
length_scale=1.0 / num_threads=2` 与 `TtsEngine.java` 一致 ⇒ 桌面试听听到的就是设备上的效果。
