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
    console.log('PASS: upload, edit undo, project roundtrip, rejected import preserves state, mobile width');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
