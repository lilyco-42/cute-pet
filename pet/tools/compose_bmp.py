# -*- coding: utf-8 -*-
"""GDI 版立绘合成: 输出小尺寸 32 位 BGRA BMP(GDI 直接加载, 无解码峰值内存)。
用法: python compose_bmp.py <face> <dress> <diff> <w> <h> <output.bmp>
"""
import json, os, sys
from PIL import Image

BASE = r'D:/Code/cute_box/pet/assets'
MANIFEST = os.path.join(BASE, 'murasame_manifest.json')
LAYER_DIR = os.path.join(BASE, 'murasame_layers')

def main():
    face = sys.argv[1] if len(sys.argv) > 1 else '01'
    dress = sys.argv[2] if len(sys.argv) > 2 else '私服'
    diff = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    out_w = int(sys.argv[4]) if len(sys.argv) > 4 else 300
    out_h = int(sys.argv[5]) if len(sys.argv) > 5 else 550
    out = sys.argv[6] if len(sys.argv) > 6 else 'pet.bmp'

    d = json.load(open(MANIFEST, encoding='utf-8'))
    s = d['sets']['a']
    items = s['composition']['items']
    groups = s['composition']['groups']

    def find_item(gid, name):
        for it in items.values():
            if it.get('group') == gid and it.get('name') == name:
                return it
        return None

    selected = []
    for dr in s['info']['dress']:
        if dr['dress'] == dress and dr['diff'] == diff:
            it = find_item(0, dr['layer'])
            if it:
                z = 2 if '髪かぶせ' in it['name'] else 0
                selected.append((it['layer_id'], z))
    for f in s['info']['face']:
        if f['face'] == face:
            gname, lname = f['layer'].split('/')
            it = find_item(groups.get(gname), lname)
            if it:
                selected.append((it['layer_id'], 1))

    min_x = min(items[str(sid)]['left'] for sid, _ in selected)
    min_y = min(items[str(sid)]['top'] for sid, _ in selected)
    max_x = max(items[str(sid)]['left'] + items[str(sid)]['w'] for sid, _ in selected)
    max_y = max(items[str(sid)]['top'] + items[str(sid)]['h'] for sid, _ in selected)
    canvas = Image.new('RGBA', (max_x - min_x, max_y - min_y), (0, 0, 0, 0))

    for sid, z in sorted(selected, key=lambda x: x[1]):
        it = items[str(sid)]
        p = os.path.join(LAYER_DIR, f'a_{sid}.png')
        if not os.path.exists(p):
            continue
        img = Image.open(p).convert('RGBA')
        canvas.alpha_composite(img, (it['left'] - min_x, it['top'] - min_y))

    # 缩放到目标尺寸 + 居中(保持比例)
    scale = min(out_w / canvas.width, out_h / canvas.height)
    nw, nh = int(canvas.width * scale), int(canvas.height * scale)
    small = canvas.resize((nw, nh), Image.LANCZOS)
    final = Image.new('RGBA', (out_w, out_h), (0, 0, 0, 0))
    final.alpha_composite(small, ((out_w - nw) // 2, (out_h - nh) // 2))

    # 保存 32 位 BGRA BMP
    final.save(out, 'BMP')
    print(f'输出: {out} {final.size} {os.path.getsize(out)}B')
    return final


def eye_patch(normal_path, blink_path, out, margin=6):
    """生成闭眼补丁: 闭眼帧与正常帧的眼睛区域差异(透明背景, 叠加用)。"""
    a = Image.open(normal_path).convert('RGBA')
    b = Image.open(blink_path).convert('RGBA')
    assert a.size == b.size
    W, H = a.size
    pa, pb = a.load(), b.load()
    min_x, min_y, max_x, max_y = W, H, 0, 0
    for y in range(0, H, 2):
        for x in range(0, W, 2):
            ra, ga, ba, aa = pa[x, y]
            rb, gb, bb, ab = pb[x, y]
            if abs(ra - rb) + abs(ga - gb) + abs(ba - bb) + abs(aa - ab) > 40:
                min_x = min(min_x, x); max_x = max(max_x, x)
                min_y = min(min_y, y); max_y = max(max_y, y)
    if max_x <= min_x:
        print('无差异区域')
        return
    min_x = max(0, min_x - margin); min_y = max(0, min_y - margin)
    max_x = min(W, max_x + margin); max_y = min(H, max_y + margin)
    patch = Image.new('RGBA', (max_x - min_x, max_y - min_y), (0, 0, 0, 0))
    for y in range(min_y, max_y):
        for x in range(min_x, max_x):
            rb, gb, bb, ab = pb[x, y]
            patch.putpixel((x - min_x, y - min_y), (rb, gb, bb, ab))
    patch.save(out, 'BMP')
    print(f'眼睛补丁: {out} {(max_x-min_x)}x{(max_y-min_y)} {os.path.getsize(out)}B @({min_x},{min_y})')
    return (min_x, min_y)


if __name__ == '__main__':
    main()
