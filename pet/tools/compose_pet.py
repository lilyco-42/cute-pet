# -*- coding: utf-8 -*-
"""预合成立绘: 按 manifest 的 dress/face 选择图层并合成单张 PNG(GDI 版用)。
用法: python compose_pet.py <face> <dress> <diff> <output.png>
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
    out = sys.argv[4] if len(sys.argv) > 4 else 'pet_composed.png'

    d = json.load(open(MANIFEST, encoding='utf-8'))
    s = d['sets']['a']
    items = s['composition']['items']
    groups = s['composition']['groups']

    def find_item(gid, name):
        for it in items.values():
            if it.get('group') == gid and it.get('name') == name:
                return it
        return None

    # 选层(模拟 main.rs selected_layers): (layer_id, z)
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
                z = 1
                selected.append((it['layer_id'], z))

    # 画布范围
    min_x = min(items[str(sid)]['left'] for sid, _ in selected)
    min_y = min(items[str(sid)]['top'] for sid, _ in selected)
    max_x = max(items[str(sid)]['left'] + items[str(sid)]['w'] for sid, _ in selected)
    max_y = max(items[str(sid)]['top'] + items[str(sid)]['h'] for sid, _ in selected)
    canvas = Image.new('RGBA', (max_x - min_x, max_y - min_y), (0, 0, 0, 0))

    for sid, z in sorted(selected, key=lambda x: x[1]):
        it = items[str(sid)]
        p = os.path.join(LAYER_DIR, f'a_{sid}.png')
        if not os.path.exists(p):
            print(f'缺层: {sid}')
            continue
        img = Image.open(p).convert('RGBA')
        canvas.alpha_composite(img, (it['left'] - min_x, it['top'] - min_y))
        print(f'合成层 {sid} z={z} {it["name"]}')

    canvas.save(out)
    print(f'输出: {out} {canvas.size} {os.path.getsize(out)}B')

if __name__ == '__main__':
    main()
