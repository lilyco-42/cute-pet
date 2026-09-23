#!/usr/bin/env python3
"""对 sid_audition 产物做基频(F0)分析, 生成可试听的 HTML 索引。

为什么: 174 个音色靠人耳逐个听太慢。先用 **基频中位数** 程序化初筛(不看脸也能
分出女声区/男声区 + 音高高低), 把女声区按音高排序排前面, 人耳只需听前 20 个
终选 —— 程序初筛 + 人耳终选, 而不是让人盲听 174 条。
"""
import glob
import html
import os
import wave

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
AUD = os.environ.get("PET_SID_OUT") or os.path.abspath(os.path.join(REPO, "..", "cute-pet-sid-audition"))
CUR_DEFAULT = 0          # OverlayService: getInt(PREF_TTS_SID, 0)
F0_MIN, F0_MAX = 70.0, 400.0


def read_wav(path):
    with wave.open(path, "rb") as fh:
        sr = fh.getframerate()
        raw = fh.readframes(fh.getnframes())
    return np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0, sr


def median_f0(x, sr):
    """自相关法估计中位基频(只统计有声且周期性够的帧)。"""
    frame = int(sr * 0.04)
    hop = int(sr * 0.02)
    lo, hi = int(sr / F0_MAX), int(sr / F0_MIN)
    voiced = []
    for start in range(0, len(x) - frame, hop):
        seg = x[start:start + frame]
        if np.sqrt(np.mean(seg ** 2)) < 0.02:       # 静音
            continue
        seg = seg - seg.mean()
        ac = np.correlate(seg, seg, mode="full")[frame - 1:]
        if ac[0] <= 0:
            continue
        ac = ac / ac[0]
        if ac[lo:hi].size == 0:
            continue
        idx = int(np.argmax(ac[lo:hi])) + lo
        if ac[idx] < 0.35:                          # 周期性不足
            continue
        voiced.append(sr / idx)
    return (float(np.median(voiced)), len(voiced)) if voiced else (None, 0)


def main():
    rows = []
    for path in sorted(glob.glob(os.path.join(AUD, "sid_*.wav"))):
        sid = int(os.path.basename(path)[4:7])
        x, sr = read_wav(path)
        f0, nv = median_f0(x, sr)
        rows.append({"sid": sid, "f0": f0, "voiced": nv,
                     "sec": len(x) / sr, "file": os.path.basename(path)})

    scored = [r for r in rows if r["f0"]]
    scored.sort(key=lambda r: r["f0"])
    female = [r for r in scored if r["f0"] >= 165]
    print(f"总 {len(rows)} 个; 有基频 {len(scored)} 个; 女声区(>=165Hz) {len(female)} 个")
    print("女声区音高排序(前 20):")
    for r in female[:20]:
        print(f"  sid={r['sid']:3d}  F0={r['f0']:.0f}Hz  时长={r['sec']:.2f}s")

    # 推荐先听: 音高落在 180-250Hz(常见年轻女声)区间, 按时长过滤掉异常短的
    rec = [r for r in female if 180 <= r["f0"] <= 250]
    def card(r):
        return (f'<div class="c"><div class="n">sid {r["sid"]:03d}'
                f'<span>{r["f0"]:.0f}Hz</span></div>'
                f'<audio controls preload="none" src="{r["file"]}"></audio></div>')

    def row(r):
        f0 = "—" if not r["f0"] else f'{r["f0"]:.0f}'
        cls = "f" if r["f0"] and r["f0"] >= 165 else "m"
        return (f'<tr class="{cls}"><td>{r["sid"]:03d}</td><td>{f0}</td>'
                f'<td><audio controls preload="none" src="{r["file"]}"></audio></td></tr>')

    table = "\n".join(row(r) for r in rows)

    doc = f"""<!DOCTYPE html>
<html lang="zh"><head><meta charset="utf-8">
<title>丛雨音色试听 · aishell3 174 speaker</title>
<style>
 body{{margin:0;padding:24px;background:#14161a;color:#e8eaed;
      font:15px/1.6 -apple-system,"Segoe UI",system-ui,sans-serif}}
 h1{{font-size:20px;margin:0 0 6px}} h2{{font-size:16px;margin:26px 0 10px;color:#9fd0ff}}
 p,li{{color:#a9b1ba;font-size:13.5px}} code{{background:#22262c;padding:1px 6px;border-radius:4px;color:#cfe3ff}}
 .meta{{color:#7d858e;font-size:13px;margin-bottom:16px}}
 .grid{{display:grid;grid-template-columns:repeat(auto-fill,minmax(230px,1fr));gap:10px}}
 .c{{background:#1c2027;border:1px solid #272c34;border-radius:10px;padding:10px}}
 .n{{font-weight:600;margin-bottom:6px;display:flex;justify-content:space-between}}
 .n span{{color:#7d858e;font-weight:400}}
 audio{{width:100%;height:34px}}
 table{{border-collapse:collapse;width:100%;font-size:13px}}
 td{{padding:4px 8px;border-bottom:1px solid #23272e}}
 tr.f td:first-child{{color:#ff9db1}} tr.m td:first-child{{color:#8fb2cc}}
 details{{margin-top:8px}} summary{{cursor:pointer;color:#9fd0ff}}
 .top{{background:#1c2027;border:1px solid #2c3138;border-radius:10px;padding:12px 14px}}
</style></head><body>
<h1>丛雨音色试听 —— aishell3 174 个 speaker</h1>
<div class="meta">模型 <code>vits-icefall-zh-aishell3</code>（与 APK 内同一份）· 合成句「<b>N，你好，我是丛雨。</b>」·
当前代码默认 <code>sid = {CUR_DEFAULT}</code></div>

<div class="top"><b>① 盲听扫描（一遍过）</b><br>
<audio controls preload="none" src="all-sids.wav" style="width:100%;margin-top:8px"></audio>
<p style="margin:8px 0 0">每个音色先说编号再说台词，听到喜欢的记下编号即可。</p></div>

<h2>② 程序初筛：女声区 + 音高 180–250Hz（推荐先听这 {len(rec)} 个）</h2>
<div class="grid">
{chr(10).join(card(r) for r in rec)}
</div>

<h2>③ 全部 174 个</h2>
<details><summary>展开全部（红=sid 音高≥165Hz 偏女声，蓝=偏低）</summary>
<table>{table}</table></details>

<h2>④ 选定后怎么生效</h2>
<p>把编号告诉 AI → 改 <code>OverlayService.getInt(PREF_TTS_SID, N)</code> 的默认值 → 重新构建 APK。</p>
</body></html>"""
    out = os.path.join(AUD, "index.html")
    with open(out, "w", encoding="utf-8") as fh:
        fh.write(doc)
    print("写出:", out)


if __name__ == "__main__":
    main()
