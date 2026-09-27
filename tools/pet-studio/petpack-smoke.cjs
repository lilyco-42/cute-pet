const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const path = require('node:path');
const os = require('node:os');
(async () => {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
    await page.goto(pathToFileURL(path.join(__dirname, 'petpack.html')).href);
    assert.equal(await page.locator('#export').isDisabled(), true, 'export stays disabled until all action strips are loaded');
    const makeStrip = async (count, tileSize = 16) => {
      const dataUrl = await page.evaluate(({ frameCount, frameSize }) => {
        const source = document.createElement('canvas');
        source.width = frameCount * frameSize;
        source.height = frameSize;
        const paint = source.getContext('2d');
        paint.fillStyle = '#d9425c';
        for (let index = 0; index < frameCount; index++) paint.fillRect(index * frameSize + 4, 4, frameSize - 8, frameSize - 8);
        return source.toDataURL('image/png');
      }, { frameCount: count, frameSize: tileSize });
      return Buffer.from(dataUrl.split(',')[1], 'base64');
    };
    await page.locator('#sheet-idle').setInputFiles({ name: 'idle.png', mimeType: 'image/png', buffer: await makeStrip(3, 17) });
    await page.getByText('待机帧带尺寸无效', { exact: false }).waitFor();
    assert.equal(await page.locator('#export').isDisabled(), true);
    const actions = [
      ['idle', 4], ['walk', 6], ['sit', 4], ['sleep', 4], ['reaction', 4],
    ];
    for (const [id, count] of actions) {
      await page.locator(`#sheet-${id}`).setInputFiles({ name: `${id}.png`, mimeType: 'image/png', buffer: await makeStrip(count) });
      await page.getByText(`已载入 ${count} 帧`, { exact: false }).waitFor();
    }
    await page.locator('#pet-id').fill('test-pet');
    await page.locator('#pet-name').fill('测试桌宠');
    await page.locator('#pet-description').fill('用于验证跨设备桌宠包导出');
    assert.equal(await page.locator('#export').isDisabled(), false);
    await page.locator('#action').selectOption('walk');
    assert.match(await page.locator('#frame-status').innerText(), /行走/);
    await page.locator('#play').click();
    assert.equal(await page.locator('#play').innerText(), '暂停');
    await page.locator('#play').click();
    const downloadPromise = page.waitForEvent('download');
    await page.locator('#export').click();
    const download = await downloadPromise;
    assert.equal(download.suggestedFilename(), 'test-pet.petpack');
    const output = path.join(process.env.RUNNER_TEMP || os.tmpdir(), 'lain42-petpack-smoke.petpack');
    await download.saveAs(output);
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'mobile layout must not overflow');
    console.log(`PASS: 5 action strips, 22 frame preview/playback, mobile layout, and exported ${output}`);
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
