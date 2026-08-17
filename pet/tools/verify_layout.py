# -*- coding: utf-8 -*-
"""模拟器截图 UI 布局验证: 非黑像素行分布 + 底部控件聚类检测"""
import sys
from collections import Counter
from PIL import Image, ImageFilter

def analyze(path):
    im = Image.open(path).convert('RGB')
    W, H = im.size
    px = im.load()
    print(f"size: {W}x{H}")

    # 非背景(非近黑)像素的行分布
    row_density = []
    for y in range(0, H, 4):
        n = 0
        for x in range(0, W, 4):
            r, g, b = px[x, y]
            if r + g + b > 60:  # 非近黑
                n += 1
        row_density.append((y, n))
    bands = []
    start = None
    for y, n in row_density:
        active = n > 0
        if active and start is None:
            start = y
        elif not active and start is not None:
            bands.append((start, y))
            start = None
    if start is not None:
        bands.append((start, H))
    print("content bands (y0,y1):", bands)

    # 底部 40% 区域内找"亮色大块"(按钮/输入框候选)
    bottom = int(H * 0.55)
    # 做一次降采样取均值
    small = im.crop((0, bottom, W, H)).resize((W // 6, (H - bottom) // 6))
    sp = small.load()
    sw, sh = small.size
    bright = []
    for y in range(sh):
        for x in range(sw):
            r, g, b = sp[x, y]
            if r + g + b > 240:  # 亮色(按钮填充/边框)
                bright.append((x * 6, bottom + y * 6))
    print("bright pixels in bottom zone:", len(bright))
    # 聚类成水平条带
    if bright:
        rows = Counter(y for _, y in bright)
        print("bottom bright row histogram (top 12):", rows.most_common(12))
        # 每行 x 范围
        xs = [x for x, _ in bright]
        print("bright x range:", min(xs), "-", max(xs))

if __name__ == '__main__':
    analyze(sys.argv[1])
