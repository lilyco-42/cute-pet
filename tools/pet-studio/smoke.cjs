const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const path = require('node:path');
(async () => {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
    await page.goto(pathToFileURL(path.join(__dirname, 'index.html')).href);
    const png = await page.evaluate(() => { const c = document.createElement('canvas'); c.width = c.height = 16; c.getContext('2d').fillRect(0, 0, 16, 16); return c.toDataURL(); });
    await page.locator('#upload').setInputFiles({ name: 'body.png', mimeType: 'image/png', buffer: Buffer.from(png.split(',')[1], 'base64') });
    await page.getByText('部件已添加', { exact: true }).waitFor();
    await page.locator('#x').fill('42'); await page.locator('#x').dispatchEvent('change');
    await page.locator('#undo').click(); assert.equal(await page.locator('#x').inputValue(), '0');
    const downloadPromise = page.waitForEvent('download'); await page.locator('#save').click();
    const download = await downloadPromise; const file = await download.path();
    await page.locator('#remove').click();
    assert.equal(await page.locator('#layers option').count(), 0);
    await page.locator('#open').setInputFiles(file); await page.getByText('项目已打开', { exact: true }).waitFor();
    assert.equal(await page.locator('#layers option').count(), 1);
    await page.locator('#open').setInputFiles({ name: 'bad.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify({ version: 1, layers: [{ src: 'https://example.com/image.png' }] })) });
    await page.getByText('项目中有无效部件', { exact: true }).waitFor();
    assert.equal(await page.locator('#layers option').count(), 1);
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));

    let modelContent = JSON.stringify({ operations: [{ index: 0, x: 42, y: 7 }] });
    let modelRequest;
    await page.route('https://api.lain42.top/v1/chat/completions', async route => {
      const request = route.request();
      const cors = {
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'POST, OPTIONS',
        'Access-Control-Allow-Headers': 'authorization, content-type'
      };
      if (request.method() === 'OPTIONS') return route.fulfill({ status: 204, headers: cors });
      modelRequest = { headers: request.headers(), body: request.postDataJSON() };
      return route.fulfill({
        status: 200,
        headers: { ...cors, 'Content-Type': 'application/json' },
        body: JSON.stringify({ choices: [{ message: { content: modelContent } }] })
      });
    });
    await page.locator('#ai-model').fill('mock/model');
    await page.locator('#ai-key').fill('test-only-secret');
    await page.locator('#ai-prompt').fill('Move the body a little right');
    await page.locator('#ai-preview').click();
    await page.getByText('AI 修改建议已生成；请检查预览后决定是否应用', { exact: true }).waitFor();
    assert.equal(await page.locator('#x').inputValue(), '0', 'proposal must not mutate the project');
    assert.equal(await page.locator('#y').inputValue(), '0');
    assert.equal(modelRequest.body.model, 'mock/model');
    assert.equal(modelRequest.headers.authorization, 'Bearer test-only-secret');
    assert.doesNotMatch(JSON.stringify(modelRequest.body), /data:image\/png;base64/, 'PNG pixels must stay local');
    const storedSecret = await page.evaluate(() => {
      try {
        const stores = [localStorage, sessionStorage];
        return stores.some(store => Array.from({ length: store.length }, (_, i) => store.key(i))
          .some(key => key && (store.getItem(key) || '').includes('test-only-secret')));
      } catch (_) { return false; }
    });
    assert.equal(storedSecret, false, 'API key must not be persisted');
    await page.locator('#ai-apply').click();
    assert.equal(await page.locator('#x').inputValue(), '42');
    assert.equal(await page.locator('#y').inputValue(), '7');
    await page.locator('#undo').click();
    assert.equal(await page.locator('#x').inputValue(), '0', 'accepted AI edits must be undoable');
    assert.equal(await page.locator('#y').inputValue(), '0');

    modelContent = JSON.stringify({ operations: [{ index: 0, visible: false }] });
    await page.locator('#ai-preview').click();
    await page.getByText('AI 修改建议已生成；请检查预览后决定是否应用', { exact: true }).waitFor();
    assert.equal(await page.locator('#visible').isChecked(), true, 'proposal must not change visibility before acceptance');
    await page.locator('#ai-cancel').click();
    assert.equal(await page.locator('#visible').isChecked(), true, 'cancel must discard the proposal');
    assert.equal(await page.locator('#proposal').isHidden(), true);

    modelContent = JSON.stringify({ operations: [{ index: 0, x: 99999 }] });
    await page.locator('#ai-preview').click();
    await page.getByText('图层坐标超出安全范围', { exact: true }).waitFor();
    assert.equal(await page.locator('#x').inputValue(), '0', 'out-of-range model output must not mutate the project');
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    console.log('PASS: upload, edit undo, project roundtrip, corrupt import, mobile layout, local-only AI preview/apply/cancel/undo, patch bounds and key storage');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
