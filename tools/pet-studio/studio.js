'use strict';
const $ = id => document.getElementById(id);
const canvas = $('preview'), ctx = canvas.getContext('2d');
let project = { version: 1, layers: [] }, selected = 0, history = [], busy = false;
const images = new Map();
function report(text) { $('status').textContent = text; }
function checkpoint() { history.push(JSON.stringify(project)); if (history.length > 20) history.shift(); }
function draw() {
  ctx.clearRect(0, 0, 768, 768);
  for (const l of project.layers) if (l.visible) {
    const im = images.get(l.src); if (im) ctx.drawImage(im, l.x, l.y, im.width * l.scale, im.height * l.scale);
  }
  const retained = new Set(project.layers.map(l => l.src));
  for (const entry of history) for (const l of JSON.parse(entry).layers) retained.add(l.src);
  for (const key of images.keys()) if (!retained.has(key)) images.delete(key);
  $('layers').replaceChildren(...project.layers.map((l, i) => new Option(l.name, String(i))));
  selected = Math.max(0, Math.min(selected, project.layers.length - 1));
  $('layers').value = String(selected);
  const l = project.layers[selected];
  for (const key of ['x', 'y', 'scale', 'visible']) {
    $(key).disabled = !l;
    if (key === 'visible') $(key).checked = l?.visible ?? false;
    else $(key).value = l?.[key] ?? '';
  }
}
function decode(src) {
  return new Promise((resolve, reject) => {
    const im = new Image();
    im.onload = () => im.width <= 2048 && im.height <= 2048 ? resolve(im) : reject(Error('图片尺寸超过 2048×2048'));
    im.onerror = () => reject(Error('无法读取 PNG 图片')); im.src = src;
  });
}
async function run(action) {
  if (busy) return; busy = true;
  try { await action(); } catch (e) { report(e.message); } finally { busy = false; draw(); }
}
$('upload').onchange = () => run(async () => {
  const f = $('upload').files[0]; if (!f) return;
  if (project.layers.length >= 12 || f.size > 2 * 1024 * 1024 || f.type !== 'image/png') throw Error('最多 12 层，单个 PNG 不超过 2 MB');
  const src = await new Promise((resolve, reject) => { const r = new FileReader(); r.onload = () => resolve(r.result); r.onerror = reject; r.readAsDataURL(f); });
  const im = await decode(src); images.set(src, im); checkpoint();
  project.layers.push({ name: f.name.slice(0, 100), src, x: 0, y: 0, scale: Math.min(1, 768 / Math.max(im.width, im.height)), visible: true });
  selected = project.layers.length - 1; report('部件已添加'); $('upload').value = '';
});
$('layers').onchange = () => { selected = Number($('layers').value); draw(); };
for (const key of ['x', 'y', 'scale', 'visible']) $(key).onchange = () => {
  if (busy || !project.layers[selected]) return;
  if (key !== 'visible' && (!$(key).value || !$(key).checkValidity())) { report('请输入范围内的数值'); draw(); return; }
  checkpoint(); project.layers[selected][key] = key === 'visible' ? $(key).checked : Number($(key).value); draw();
};
$('up').onclick = () => { if (busy || selected >= project.layers.length - 1) return; checkpoint(); const a = project.layers; [a[selected], a[selected + 1]] = [a[selected + 1], a[selected]]; selected++; draw(); };
$('remove').onclick = () => { if (busy || !project.layers[selected]) return; checkpoint(); project.layers.splice(selected, 1); draw(); };
$('undo').onclick = () => { if (!busy && history.length) { project = JSON.parse(history.pop()); draw(); } };
function download(blob, name) { const url = URL.createObjectURL(blob), a = document.createElement('a'); a.href = url; a.download = name; a.click(); setTimeout(() => URL.revokeObjectURL(url), 10000); }
$('save').onclick = () => download(new Blob([JSON.stringify(project)], { type: 'application/json' }), 'character.json');
$('png').onclick = () => canvas.toBlob(blob => { if (blob) download(blob, 'character.png'); });
$('open').onchange = () => run(async () => {
  const f = $('open').files[0]; if (!f) return;
  if (f.size > 34 * 1024 * 1024) throw Error('项目文件超过 34 MB');
  const p = JSON.parse(await f.text());
  if (p.version !== 1 || !Array.isArray(p.layers) || p.layers.length > 12) throw Error('不支持的项目格式');
  const clean = [], decoded = new Map();
  for (const l of p.layers) {
    if (!l || typeof l.name !== 'string' || l.name.length > 100 || typeof l.visible !== 'boolean' ||
      typeof l.src !== 'string' || l.src.length > 2800000 || !/^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(l.src) ||
      !Number.isFinite(l.x) || Math.abs(l.x) > 2048 || !Number.isFinite(l.y) || Math.abs(l.y) > 2048 ||
      !Number.isFinite(l.scale) || l.scale < .05 || l.scale > 4) throw Error('项目中有无效部件');
    decoded.set(l.src, await decode(l.src));
    clean.push({ name: l.name, src: l.src, x: l.x, y: l.y, scale: l.scale, visible: l.visible });
  }
  checkpoint(); project = { version: 1, layers: clean }; for (const [src, im] of decoded) images.set(src, im);
  selected = 0; report('项目已打开'); $('open').value = '';
});
draw();
