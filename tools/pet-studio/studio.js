'use strict';
const $ = id => document.getElementById(id);
const canvas = $('preview'), ctx = canvas.getContext('2d');
let project = { version: 1, layers: [] }, selected = 0, history = [], busy = false, proposal = null;
const images = new Map();
function report(text) { $('status').textContent = text; }
function checkpoint() { history.push(JSON.stringify(project)); if (history.length > 20) history.shift(); }
function updateLetsGalExportButton() {
  const hasVisibleLayer = project.layers.some(layer => layer.visible);
  const hasNames = $('letsgal-name').value.trim().length > 0 && $('letsgal-expression').value.trim().length > 0;
  $('letsgal-export').disabled = busy || !hasVisibleLayer || !hasNames;
}
function drawProject(target, state, targetContext) {
  const paint = targetContext || target.getContext('2d');
  paint.clearRect(0, 0, target.width, target.height);
  for (const layer of state.layers) if (layer.visible) {
    const image = images.get(layer.src);
    if (image) paint.drawImage(image, layer.x, layer.y, image.width * layer.scale, image.height * layer.scale);
  }
}
function draw() {
  drawProject(canvas, project, ctx);
  updateLetsGalExportButton();
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
  $('proposal').hidden = !proposal;
  if (proposal) {
    drawProject($('proposal-canvas'), proposal.candidate);
    const stale = JSON.stringify(project) !== proposal.base;
    $('proposal-warning').textContent = stale ? '项目已变化，此建议已过期，请重新生成。' : '图片仍只在本机。应用后仍可撤销。';
    $('ai-apply').disabled = stale;
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
  const files = Array.from($('upload').files); if (!files.length) return;
  if (project.layers.length + files.length > 12) throw Error('最多添加 12 个部件');
  const staged = [];
  for (const file of files) {
    if (file.size > 2 * 1024 * 1024 || file.type !== 'image/png') throw Error('每个部件都必须是小于 2 MB 的 PNG');
    const src = await new Promise((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(reader.result); reader.onerror = reject; reader.readAsDataURL(file); });
    const image = await decode(src);
    staged.push({ layer: { name: file.name.slice(0, 100), src, x: 0, y: 0, scale: Math.min(1, 768 / Math.max(image.width, image.height)), visible: true }, image });
  }
  checkpoint();
  for (const item of staged) { images.set(item.layer.src, item.image); project.layers.push(item.layer); }
  selected = project.layers.length - 1; report(staged.length === 1 ? '部件已添加' : '已添加 ' + staged.length + ' 个部件'); $('upload').value = '';
});
$('layers').onchange = () => { selected = Number($('layers').value); draw(); };
for (const key of ['x', 'y', 'scale', 'visible']) $(key).onchange = () => {
  if (busy || !project.layers[selected]) return;
  if (key !== 'visible' && (!$(key).value || !$(key).checkValidity())) { report('请输入范围内的数值'); draw(); return; }
  const value = key === 'visible' ? $(key).checked : Number($(key).value);
  if (project.layers[selected][key] === value) return;
  checkpoint(); project.layers[selected][key] = value; draw();
};
$('up').onclick = () => { if (busy || selected >= project.layers.length - 1) return; checkpoint(); const a = project.layers; [a[selected], a[selected + 1]] = [a[selected + 1], a[selected]]; selected++; draw(); };
$('remove').onclick = () => { if (busy || !project.layers[selected]) return; checkpoint(); project.layers.splice(selected, 1); draw(); };
$('undo').onclick = () => { if (!busy && history.length) { project = JSON.parse(history.pop()); draw(); } };
function download(blob, name) { const url = URL.createObjectURL(blob), a = document.createElement('a'); a.href = url; a.download = name; a.click(); setTimeout(() => URL.revokeObjectURL(url), 10000); }
$('ai-preview').onclick = () => run(async () => {
  if (!project.layers.length) throw Error('先添加至少一个 PNG 部件');
  const endpointText = $('ai-base').value.trim(), model = $('ai-model').value.trim(), key = $('ai-key').value.trim(), instruction = $('ai-prompt').value.trim();
  if (!endpointText || !model || !key || !instruction) throw Error('请填写 API 地址、模型、API Key 和修改要求');
  let endpoint;
  try { endpoint = new URL(endpointText); } catch (_) { throw Error('请输入有效的 API 地址'); }
  if (endpoint.protocol !== 'https:' && !['localhost', '127.0.0.1'].includes(endpoint.hostname)) throw Error('API 地址必须使用 HTTPS');
  if (endpoint.username || endpoint.password || endpoint.hash || endpoint.search) throw Error('API 地址不能包含账号、密码、查询参数或片段');
  const base = endpoint.href.replace(/\/+$/, '');
  proposal = null;
  draw();
  report('正在请求模型建议…');
  const before = JSON.stringify(project);
  const layers = project.layers.map((layer, index) => ({ index, name: layer.name, x: layer.x, y: layer.y, scale: layer.scale, visible: layer.visible }));
  let response;
  try {
    response = await fetch(base + '/chat/completions', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + key },
      body: JSON.stringify({
        model,
        temperature: 0,
        messages: [
          { role: 'system', content: 'Return only a JSON object: {"operations":[{"index":0,"x":12,"y":20,"scale":0.9,"visible":true}]}. You may only change x, y, scale, and visible for existing layer indexes. Omit unchanged fields. Never add, delete, rename, reorder layers, execute code, or return image data. Use valid indexes from the provided list. Coordinates must be between -2048 and 2048; scale between 0.05 and 4.' },
          { role: 'user', content: JSON.stringify({ instruction, existing_layers: layers }) }
        ]
      })
    });
  } catch (_) {
    throw Error('无法连接模型服务；请检查地址、网络和服务端 CORS 设置');
  }
  if (!response.ok) throw Error('模型请求失败（HTTP ' + response.status + '）；请检查模型、额度和 API Key');
  let payload;
  try { payload = await response.json(); } catch (_) { throw Error('模型服务返回的内容不是有效 JSON'); }
  const content = payload?.choices?.[0]?.message?.content;
  if (typeof content !== 'string') throw Error('模型没有返回可读取的修改建议');
  let answer;
  const json = content.trim().replace(/^```(?:json)?\s*/i, '').replace(/\s*```$/, '');
  try { answer = JSON.parse(json); } catch (_) { throw Error('模型没有返回有效 JSON，请重试'); }
  if (!Array.isArray(answer.operations) || answer.operations.length < 1 || answer.operations.length > project.layers.length) throw Error('建议必须只包含现有图层的修改');
  const candidate = JSON.parse(before), changed = new Set(), allowed = new Set(['index', 'x', 'y', 'scale', 'visible']);
  const fields = ['x', 'y', 'scale', 'visible'];
  for (const operation of answer.operations) {
    if (!operation || typeof operation !== 'object' || Array.isArray(operation) || Object.keys(operation).some(field => !allowed.has(field))) throw Error('建议包含不支持的字段');
    const index = operation.index;
    if (!Number.isInteger(index) || index < 0 || index >= candidate.layers.length || changed.has(index)) throw Error('建议引用了不存在或重复的图层');
    const keys = fields.filter(field => Object.prototype.hasOwnProperty.call(operation, field));
    if (!keys.length) throw Error('建议没有可应用的图层修改');
    for (const field of keys) {
      const value = operation[field];
      if (field === 'visible') {
        if (typeof value !== 'boolean') throw Error('显隐修改必须为 true 或 false');
      } else if (!Number.isFinite(value)) throw Error('位置和缩放必须是有效数字');
      if ((field === 'x' || field === 'y') && Math.abs(value) > 2048) throw Error('图层坐标超出安全范围');
      if (field === 'scale' && (value < 0.05 || value > 4)) throw Error('图层缩放超出安全范围');
      candidate.layers[index][field] = value;
    }
    if (keys.every(field => candidate.layers[index][field] === project.layers[index][field])) throw Error('建议没有产生任何变化');
    changed.add(index);
  }
  proposal = { base: before, candidate };
  const labels = { x: '水平位置', y: '垂直位置', scale: '缩放', visible: '显示部件' };
  const diff = [];
  for (const index of changed) for (const field of fields) {
    const oldValue = project.layers[index][field], newValue = candidate.layers[index][field];
    if (oldValue !== newValue) {
      const row = document.createElement('li');
      row.textContent = project.layers[index].name + '：' + labels[field] + ' ' + oldValue + ' → ' + newValue;
      diff.push(row);
    }
  }
  $('proposal-diff').replaceChildren(...diff);
  draw();
  report('AI 修改建议已生成；请检查预览后决定是否应用');
});
$('ai-apply').onclick = () => {
  if (busy || !proposal) return;
  if (JSON.stringify(project) !== proposal.base) { report('项目已变化，请重新生成建议'); draw(); return; }
  checkpoint(); project = proposal.candidate; proposal = null; draw(); report('已应用 AI 建议，可撤销');
};
$('ai-cancel').onclick = () => { if (busy) return; proposal = null; draw(); report('已放弃 AI 建议'); };
$('save').onclick = () => download(new Blob([JSON.stringify(project)], { type: 'application/json' }), 'character.json');
$('png').onclick = () => canvas.toBlob(blob => { if (blob) download(blob, 'character.png'); });
function createCharacterId() {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
$('letsgal-name').addEventListener('input', updateLetsGalExportButton);
$('letsgal-expression').addEventListener('input', updateLetsGalExportButton);
$('letsgal-export').onclick = () => run(async () => {
  if (!project.layers.some(layer => layer.visible)) throw Error('请先添加并显示至少一个 PNG 部件');
  const name = $('letsgal-name').value.trim();
  const expression = $('letsgal-expression').value.trim();
  if (!name || name.length > 80) throw Error('角色名称需为 1–80 个字符');
  if (!expression || expression.length > 80) throw Error('表情名称需为 1–80 个字符');
  report('正在本机合成立绘并打包…');
  const portrait = await new Promise((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(Error('无法生成透明立绘 PNG')), 'image/png'));
  const characterId = createCharacterId();
  const assetPath = `characters/${characterId}/portrait.png`;
  const character = { id: characterId, name, expressions: [{ name: expression, assetPath }] };
  const fragment = {
    format: 'lain42.letsgal-character-fragment',
    version: 1,
    character,
  };
  const guide = `# 导入到 LetsGal Studio\n\n1. 将本包内的 assets/ 目录复制到现有游戏工程根目录，合并文件夹，不要覆盖整个工程。\n2. 打开 characters.fragment.json，将其中的 character 对象合并进工程 characters.json 的 characters 数组；已有同名角色时，把 expressions 中的表情合并进去。\n3. 在 letsgal-ai 工程目录运行：\n\n   \`letsgal-ai register-asset <工程目录> --rel ${assetPath}\`\n\n4. 运行 \`letsgal-ai validate <工程目录>\` 检查工程。\n\n角色图片在浏览器本机合成；本包只包含 PNG、角色映射和本说明，不会改写游戏工程或执行脚本。`;
  const zip = await window.Lain42Zip.createStoredZip([
    { name: `assets/${assetPath}`, blob: portrait },
    { name: 'characters.fragment.json', blob: new Blob([JSON.stringify(fragment, null, 2) + '\n'], { type: 'application/json' }) },
    { name: 'IMPORT.md', blob: new Blob([guide], { type: 'text/markdown;charset=utf-8' }) },
  ]);
  download(zip, 'letsgal-character.zip');
  report('已下载 Let’sGal 角色包。解压后按 IMPORT.md 合并映射并登记素材；游戏工程未被修改。');
});
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
window.addEventListener('pagehide', () => { $('ai-key').value = ''; });
