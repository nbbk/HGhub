// Local fixtures only. Chromium iPhone emulation checks branching, not real iOS decoding.
// Run: BROWSER_EXECUTABLE=/usr/bin/chromium node tests/web/frontend.mjs
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { chromium } from 'playwright-core';
process.env.VITE_WEB = 'true';
const root = fileURLToPath(new URL('../../', import.meta.url));
const vite = await createServer({ root, server: { host: '127.0.0.1', port: 0, strictPort: false } });
let browser;
const passed = [];
try {
  await vite.listen();
  const origin = vite.resolvedUrls.local[0];
  browser = await chromium.launch({ headless: true, executablePath: process.env.BROWSER_EXECUTABLE || undefined, args: ['--no-sandbox'] });
  const page = await browser.newPage();
  let authenticated = false, apiCalls = 0;
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('**/auth/**', async route => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/auth/login') {
      if (route.request().postDataJSON().password !== 'fixture-only-password') return route.fulfill({ status: 403, json: { error: 'Invalid password' } });
      authenticated = true;
    }
    if (path === '/auth/logout') authenticated = false;
    await route.fulfill({ json: { authenticated } });
  });
  await page.route('**/api/**', route => {
    apiCalls++;
    if (!authenticated) return route.fulfill({ status: 401, json: { error: 'Unauthorized' } });
    const cmd = new URL(route.request().url()).pathname.split('/').pop();
    return route.fulfill({ json: cmd === 'app_boot'
      ? { user: null, logged_in: false, base: 'https://example.invalid', strict_security: true }
      : { items: [], pagination: { page: 1, total_pages: 0, total: 0 }, base: 'https://example.invalid' } });
  });
  await page.goto(origin);
  const gate = page.getByRole('heading', { name: 'HGhub 个人网页版' });
  await gate.waitFor();
  assert.equal(apiCalls, 0, 'No application API before authentication');
  await page.getByLabel('访问密码', { exact: true }).fill('wrong');
  await page.getByRole('button', { name: '进入', exact: true }).click();
  await page.getByRole('alert').waitFor();
  assert.equal(apiCalls, 0);
  passed.push('Initial access gate and wrong password do not mount application');
  const login = async () => {
    await page.getByLabel('访问密码', { exact: true }).fill('fixture-only-password');
    await page.getByRole('button', { name: '进入', exact: true }).click();
    await page.getByRole('navigation', { name: '主导航' }).waitFor();
  };
  await login();
  assert(apiCalls > 0);
  assert.equal(await page.evaluate(() => Object.values(localStorage).some(v => v.includes('fixture-only-password'))), false);
  await page.getByRole('button', { name: '设置', exact: true }).click();
  await page.getByText('个人服务器', { exact: true }).waitFor();
  assert.equal(await page.getByText('严格安全模式', { exact: true }).count(), 0);
  await page.getByRole('button', { name: '退出网页访问' }).click();
  await gate.waitFor();
  passed.push('Login, browser shell, safe web settings, logout, no password in localStorage');
  await login();
  authenticated = false;
  await page.evaluate(async () => {
    const { invoke } = await import('/src/transport.ts');
    await invoke('api_feed').catch(() => {});
  });
  await gate.waitFor();
  passed.push('API 401 automatically unmounts application and re-locks');
  assert.deepEqual(errors, []);
  await page.close();

  const mobile = await browser.newContext({
    viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true,
    userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
  });
  const phone = await mobile.newPage();
  await phone.route('**/auth/status', route => route.fulfill({ json: { authenticated: false } }));
  await phone.route('**/media/**', route => route.fulfill({ status: 404, body: '' }));
  await phone.addInitScript(() => {
    HTMLMediaElement.prototype.canPlayType = function(type) { return type.includes('mpegurl') ? 'probably' : ''; };
  });
  await phone.goto(origin);
  await phone.getByRole('heading', { name: 'HGhub 个人网页版' }).waitFor();
  const nativeHls = await phone.evaluate(async () => {
    const React = await import('/node_modules/.vite/deps/react.js');
    const { createRoot } = await import('/node_modules/.vite/deps/react-dom_client.js');
    const { default: VideoSurface } = await import('/src/components/VideoSurface.tsx');
    const host = document.createElement('div'); document.body.appendChild(host);
    createRoot(host).render(React.createElement(VideoSurface, { src: 'https://example.invalid/test.m3u8', autoPlay: false, active: false }));
    for (let i = 0; i < 30; i++) {
      const v = host.querySelector('video');
      if (v?.getAttribute('src')) return v.getAttribute('src');
      await new Promise(r => setTimeout(r, 50));
    }
    return null;
  });
  assert.equal(nativeHls, '/media/hls?u=https%3A%2F%2Fexample.invalid%2Ftest.m3u8');
  passed.push('iPhone UA + native HLS capability selects native video src, not hls.js blob');
  await mobile.close();
  console.log(JSON.stringify({ passed, limitation: 'Mock APIs; iPhone emulation only, no real iOS or upstream playback verification' }, null, 2));
} finally {
  await browser?.close(); await vite.close();
}
