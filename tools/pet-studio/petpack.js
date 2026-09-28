'use strict';
const ACTIONS = [
  { id: 'idle', label: '待机', count: 4, durations: [420, 300, 260, 420], loop: true },
  { id: 'walk', label: '行走', count: 6, durations: [130, 130, 130, 130, 130, 130], loop: true },
  { id: 'sit', label: '坐下', count: 4, durations: [220, 220, 260, 2000], loop: false, holdLastFrame: true },
  { id: 'sleep', label: '睡觉', count: 4, durations: [650, 650, 420, 650], loop: true, scale: 0.88 },
  { id: 'reaction', label: '互动', count: 4, durations: [160, 180, 220, 1500], loop: false, holdLastFrame: true },
];
const MAX_SHEET_BYTES = 8 * 1024 * 1024;
const MAX_TOTAL_BYTES = 24 * 1024 * 1024;
const PNG_SIGNATURE = [137, 80, 78, 71, 13, 10, 26, 10];
const sets = new Map();
const canvas = document.getElementById('preview');
const context = canvas.getContext('2d');
let frameIndex = 0;
let playing = false;
let previousTick = 0;
let elapsed = 0;
let animationFrame = 0;

function setStatus(message, error = false) {
  const node = document.getElementById('status');
  node.textContent = message;
  node.style.color = error ? '#a32929' : '#305d27';
}

function fileIsPng(file) {
  return file.type === 'image/png' || file.name.toLowerCase().endsWith('.png');
}

async function canvasToPng(target) {
  return new Promise((resolve, reject) => target.toBlob(blob => blob ? resolve(blob) : reject(new Error('无法生成 PNG 帧')), 'image/png'));
}

function validatePixels(imageData, action, frame) {
  const pixels = imageData.data;
  if (pixels[3] > 8) throw new Error(`${action.label}第 ${frame + 1} 帧左上角不透明；请使用带透明背景的 PNG`);
  let visible = 0;
  let green = 0;
  for (let i = 0; i < pixels.length; i += 4) {
    const red = pixels[i];
    const greenChannel = pixels[i + 1];
    const blue = pixels[i + 2];
    const alpha = pixels[i + 3];
    if (alpha > 25) {
      visible++;
      if (greenChannel > 190 && greenChannel > red * 1.7 && greenChannel > blue * 1.7) green++;
    }
  }
  if (!visible) throw new Error(`${action.label}第 ${frame + 1} 帧为空`);
  if (green / visible > 0.005) throw new Error(`${action.label}第 ${frame + 1} 帧含绿色幕布残留`);
  return visible;
}

async function splitSheet(action, file) {
  if (!fileIsPng(file)) throw new Error(`${action.label}帧带必须是 PNG`);
  if (file.size > MAX_SHEET_BYTES) throw new Error(`${action.label}帧带超过 8 MB`);
  const totalBytes = [...sets.entries()].reduce((sum, [id, set]) => sum + (id === action.id ? 0 : set.sourceBytes), file.size);
  if (totalBytes > MAX_TOTAL_BYTES) throw new Error('5 条帧带合计不能超过 24 MB');
  const signature = new Uint8Array(await file.slice(0, 8).arrayBuffer());
  if (!PNG_SIGNATURE.every((value, index) => signature[index] === value)) throw new Error(`${action.label}文件不是有效 PNG`);
  if (typeof createImageBitmap !== 'function') throw new Error('当前浏览器不支持本地图片解码，请更新浏览器');

  let source;
  const frames = [];
  try {
    source = await createImageBitmap(file);
    if (source.width > 2048 || source.height > 512 || source.height < 16 || source.width % action.count !== 0) {
      throw new Error(`${action.label}帧带尺寸无效：需要横向均分为 ${action.count} 格，单格宽高为 16–512 像素`);
    }
    const frameWidth = source.width / action.count;
    const frameHeight = source.height;
    if (frameWidth < 16 || frameWidth > 512 || frameHeight > 512) throw new Error(`${action.label}单帧尺寸必须在 16–512 像素内`);
    const otherSet = [...sets.entries()].find(([id]) => id !== action.id)?.[1];
    if (otherSet && (otherSet.frameWidth !== frameWidth || otherSet.frameHeight !== frameHeight)) {
      throw new Error(`所有动作格子需同尺寸；已载入为 ${otherSet.frameWidth}×${otherSet.frameHeight}`);
    }

    const frameCanvas = document.createElement('canvas');
    frameCanvas.width = frameWidth;
    frameCanvas.height = frameHeight;
    const frameContext = frameCanvas.getContext('2d', { willReadFrequently: true });
    for (let index = 0; index < action.count; index++) {
      frameContext.clearRect(0, 0, frameWidth, frameHeight);
      frameContext.drawImage(source, index * frameWidth, 0, frameWidth, frameHeight, 0, 0, frameWidth, frameHeight);
      const alphaArea = validatePixels(frameContext.getImageData(0, 0, frameWidth, frameHeight), action, index);
      const blob = await canvasToPng(frameCanvas);
      frames.push({ blob, image: await createImageBitmap(blob), duration: action.durations[index], alphaArea });
    }
    return { frames, frameWidth, frameHeight, sourceBytes: file.size };
  } catch (error) {
    for (const frame of frames) frame.image.close();
    throw error;
  } finally {
    source?.close();
  }
}

function selectedAction() {
  return ACTIONS.find(action => action.id === document.getElementById('action').value) || ACTIONS[0];
}

function currentFrame() {
  return sets.get(selectedAction().id)?.frames[frameIndex] || null;
}

function drawPreview() {
  context.clearRect(0, 0, canvas.width, canvas.height);
  const frame = currentFrame();
  if (!frame) {
    document.getElementById('frame-status').textContent = '载入帧带后即可预览。';
    return;
  }
  const scale = Math.min((canvas.width * 0.78) / frame.image.width, (canvas.height * 0.78) / frame.image.height);
  const width = frame.image.width * scale;
  const height = frame.image.height * scale;
  context.imageSmoothingEnabled = false;
  context.drawImage(frame.image, (canvas.width - width) / 2, (canvas.height - height) / 2, width, height);
  document.getElementById('frame-status').textContent = `${selectedAction().label} · 第 ${frameIndex + 1}/${selectedAction().count} 帧`;
}

function updateActionOptions() {
  const select = document.getElementById('action');
  const previous = select.value || 'idle';
  select.replaceChildren(...ACTIONS.map(action => new Option(`${action.label} · ${sets.has(action.id) ? action.count + ' 帧' : '待载入'}`, action.id)));
  select.value = previous;
  select.disabled = sets.size === 0;
  document.getElementById('play').disabled = !sets.has(select.value);
  if (!sets.has(select.value) && sets.size > 0) {
    select.value = sets.keys().next().value;
    document.getElementById('play').disabled = false;
  }
  drawPreview();
}

function updateExportButton() {
  const id = document.getElementById('pet-id').value.trim();
  const name = document.getElementById('pet-name').value.trim();
  const validId = /^[a-z0-9][a-z0-9-]{1,47}$/.test(id);
  const validName = name.length > 0 && name.length <= 80;
  document.getElementById('export').disabled = sets.size !== ACTIONS.length || !validId || !validName;
}

for (const action of ACTIONS) {
  const input = document.getElementById(`sheet-${action.id}`);
  input.addEventListener('change', async () => {
    const file = input.files?.[0];
    if (!file) return;
    input.disabled = true;
    setStatus(`正在本机切分${action.label}帧…`);
    try {
      const next = await splitSheet(action, file);
      const previous = sets.get(action.id);
      if (previous) for (const frame of previous.frames) frame.image.close();
      sets.set(action.id, next);
      document.getElementById(`count-${action.id}`).textContent = `已载入 ${next.frames.length} 帧 · ${next.frameWidth}×${next.frameHeight}`;
      frameIndex = 0;
      setStatus(`已载入 ${sets.size}/5 个动作，共 ${[...sets.values()].reduce((sum, set) => sum + set.frames.length, 0)} 帧。`);
      updateActionOptions();
      updateExportButton();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : '无法读取这条帧带', true);
    } finally {
      input.disabled = false;
      input.value = '';
    }
  });
}

document.getElementById('action').addEventListener('change', () => {
  frameIndex = 0;
  elapsed = 0;
  document.getElementById('play').disabled = !sets.has(selectedAction().id);
  drawPreview();
});

document.getElementById('play').addEventListener('click', () => {
  playing = !playing;
  elapsed = 0;
  previousTick = 0;
  document.getElementById('play').textContent = playing ? '暂停' : '播放';
  if (playing) animationFrame = requestAnimationFrame(tick);
  else cancelAnimationFrame(animationFrame);
});

function tick(now) {
  if (!playing) return;
  const action = selectedAction();
  const set = sets.get(action.id);
  if (!set) { playing = false; return; }
  if (previousTick) elapsed += now - previousTick;
  previousTick = now;
  while (elapsed >= set.frames[frameIndex].duration) {
    elapsed -= set.frames[frameIndex].duration;
    if (frameIndex + 1 < set.frames.length) frameIndex++;
    else if (action.loop) frameIndex = 0;
    else { frameIndex = set.frames.length - 1; playing = false; document.getElementById('play').textContent = '播放'; break; }
  }
  drawPreview();
  if (playing) animationFrame = requestAnimationFrame(tick);
}

function triggerDownload(blob, filename) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 30000);
}

async function buildPetpack() {
  const id = document.getElementById('pet-id').value.trim();
  const name = document.getElementById('pet-name').value.trim();
  const description = document.getElementById('pet-description').value.trim();
  if (!/^[a-z0-9][a-z0-9-]{1,47}$/.test(id)) throw new Error('包 ID 需为 2–48 位小写字母、数字或连字符');
  if (!name || name.length > 80) throw new Error('名称需为 1–80 个字符');
  if (description.length > 500) throw new Error('介绍不能超过 500 个字符');
  if (sets.size !== ACTIONS.length) throw new Error('请先载入全部 5 个动作帧带');
  const areas = [...sets.values()].flatMap(set => set.frames.map(frame => frame.alphaArea));
  if (Math.max(...areas) / Math.min(...areas) > 1.08) throw new Error('角色大小在不同帧之间变化超过 8%；请先统一动作帧中的角色比例');

  const animations = {};
  const entries = [];
  const idle = sets.get('idle');
  entries.push({ name: 'preview.png', blob: idle.frames[0].blob });
  for (const action of ACTIONS) {
    const set = sets.get(action.id);
    const paths = [];
    for (let index = 0; index < set.frames.length; index++) {
      const framePath = `animations/${action.id}/frame-${String(index + 1).padStart(2, '0')}.png`;
      paths.push(framePath);
      entries.push({ name: framePath, blob: set.frames[index].blob });
    }
    animations[action.id] = {
      frames: paths,
      durations: action.durations,
      loop: action.loop,
      ...(action.holdLastFrame ? { holdLastFrame: true } : {}),
      ...(action.scale ? { scale: action.scale } : {}),
    };
  }
  const manifest = {
    schemaVersion: 1,
    packageVersion: '1.0.0',
    id,
    name,
    description,
    personality: [],
    defaultSize: 'small',
    preview: 'preview.png',
    normalizationMetric: 'alpha-area-v1',
    animations,
    behavior: { random: [
      { state: 'walk', weight: 50, minDuration: 1500, maxDuration: 4200 },
      { state: 'sit', weight: 20, minDuration: 4200, maxDuration: 6200 },
      { state: 'reaction', weight: 16, minDuration: 2200, maxDuration: 3400 },
      { state: 'sleep', weight: 14, minDuration: 6800, maxDuration: 11000 },
    ] },
  };
  entries.unshift({ name: 'pet.json', blob: new Blob([JSON.stringify(manifest, null, 2) + '\n'], { type: 'application/json' }) });
  return window.Lain42Zip.createStoredZip(entries);
}

document.getElementById('export').addEventListener('click', async () => {
  const button = document.getElementById('export');
  button.disabled = true;
  try {
    setStatus('正在本机校验并打包 22 帧…');
    const blob = await buildPetpack();
    triggerDownload(blob, `${document.getElementById('pet-id').value.trim()}.petpack`);
    setStatus(`已导出桌宠包（${(blob.size / 1024 / 1024).toFixed(2)} MB）；素材未上传。`);
  } catch (error) {
    setStatus(error instanceof Error ? error.message : '无法创建桌宠包', true);
  } finally {
    updateExportButton();
  }
});

for (const id of ['pet-id', 'pet-name', 'pet-description']) document.getElementById(id).addEventListener('input', updateExportButton);
updateActionOptions();
updateExportButton();
window.addEventListener('pagehide', () => {
  cancelAnimationFrame(animationFrame);
  for (const set of sets.values()) for (const frame of set.frames) frame.image.close();
});
