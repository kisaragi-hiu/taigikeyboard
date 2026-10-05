// usage: node render.mjs stills t1,t2,... outdir   |   node render.mjs video out.mp4 [fps] [t0] [t1]
import { chromium } from 'playwright-core';
import { spawn } from 'child_process';
import fs from 'fs';
const dir = new URL('.', import.meta.url).pathname;
const [,, mode, a, b, c, d] = process.argv;
const browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  args: ['--allow-file-access-from-files', '--font-render-hinting=none', '--force-color-profile=srgb'] });
const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
page.on('console', m => console.log('[page]', m.text())); page.on('pageerror', e => console.log('[err]', e.message));
await page.goto('file://' + dir + 'index.html'); await page.evaluate(() => window.ready);
const dur = await page.evaluate(() => window.DURATION);
fs.writeFileSync(dir + 'events.json', JSON.stringify(await page.evaluate(() => window.EVENTS), null, 0));
if (mode === 'stills') {
  fs.mkdirSync(b, { recursive: true });
  for (const t of a.split(',').map(Number)) { await page.evaluate(t => render(t), t); await page.screenshot({ path: `${b}/t${t.toFixed(2)}.png` }); }
} else {
  const fps = +(b || 60), t0 = +(c || 0), t1 = +(d || dur);
  const ff = spawn('ffmpeg', ['-y', '-loglevel', 'error', '-f', 'image2pipe', '-framerate', String(fps), '-c:v', 'png', '-i', '-',
    '-c:v', 'libx264', '-preset', 'slow', '-crf', '14', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', a], { stdio: ['pipe', 'inherit', 'inherit'] });
  const n = Math.round((t1 - t0) * fps); const st = Date.now();
  for (let i = 0; i < n; i++) {
    await page.evaluate(t => render(t), t0 + i / fps);
    const buf = await page.screenshot({ type: 'png' });
    if (!ff.stdin.write(buf)) await new Promise(r => ff.stdin.once('drain', r));
    if (i % 300 === 0) console.log(`frame ${i}/${n} ${((Date.now() - st) / 1000).toFixed(0)}s`);
  }
  ff.stdin.end(); await new Promise(r => ff.on('close', r));
}
await browser.close();
