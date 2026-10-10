/**
 * End-to-end verification of the browser integration in a real Chromium.
 *
 * What is proven here (automatable):
 *   1. the extension loads and its service worker starts;
 *   2. Chrome spawns `unbundio-vault` as a Native Messaging host;
 *   3. the extension's own client (native.js → uvNativeCall) gets real answers
 *      from the encrypted vault through the stdio protocol;
 *   4. the content script fills a real login form's DOM;
 *   5. the popup renders entries, the audit summary and the generator.
 *
 * What is NOT automatable (documented, verified manually once):
 *   the toolbar click itself. `activeTab` is granted only by a real user
 *   gesture on the action, so `chrome.scripting.executeScript` from the
 *   service worker is refused by Chrome by design. Clicking the toolbar icon
 *   in a normal Chrome window is the only way to exercise that hop — see
 *   INSTALL.ko.md.
 *
 * Usage:  node tests/browser-fill.mjs
 */
import { createHash } from 'node:crypto';
import { mkdtempSync, writeFileSync, mkdirSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { execFileSync } from 'node:child_process';

const ROOT = resolve(import.meta.dirname, '..');
const EXT_DIR = join(ROOT, 'extension');
const BIN = join(ROOT, 'target', 'debug', 'unbundio-vault');
const MASTER_PW = 'e2e-master-pw-9xQ';

/** Playwright is a dev-only dependency; find it wherever it lives. */
async function loadPlaywright() {
  const roots = [
    join(ROOT, 'node_modules'),
    process.env.PLAYWRIGHT_DIR,
    process.env.HOME && join(process.env.HOME, 'dev/unbundio/keep-my-password/node_modules'),
    process.env.HOME && join(process.env.HOME, 'dev/unbundio/web-builder/admin-web/node_modules'),
  ].filter(Boolean);
  const candidates = ['playwright'];
  for (const r of roots) {
    candidates.push(join(r, 'playwright', 'index.mjs'));
    candidates.push(join(r, 'playwright', 'index.js'));
  }
  for (const c of candidates) {
    try {
      const spec = c.startsWith('/') ? pathToFileURL(c).href : c;
      const mod = await import(spec);
      if (mod.chromium) return mod;
    } catch (_e) {
      /* next candidate */
    }
  }
  return null;
}

let failures = 0;
const check = (name, ok, extra = '') => {
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${extra ? ` — ${extra}` : ''}`);
  if (!ok) failures += 1;
};

/** Chrome derives an unpacked extension's id from its absolute path:
 *  sha256(path) -> first 16 bytes -> hex digits mapped a..p. */
function unpackedExtensionId(path) {
  const digest = createHash('sha256').update(path, 'utf8').digest();
  let id = '';
  for (const byte of digest.subarray(0, 16)) {
    id += String.fromCharCode(97 + (byte >> 4));
    id += String.fromCharCode(97 + (byte & 0x0f));
  }
  return id;
}

const pw = await loadPlaywright();
if (!pw) {
  console.error('playwright not found — skipping browser e2e (npm i -D playwright).');
  process.exit(2);
}
const { chromium } = pw;

const work = mkdtempSync(join(tmpdir(), 'uv-e2e-'));
const vault = join(work, 'vault.vault');
const profile = join(work, 'profile');
const nmDir = join(profile, 'NativeMessagingHosts');
const extId = unpackedExtensionId(EXT_DIR);

// Never hang a headed browser on a regression.
const watchdog = setTimeout(() => {
  console.error('harness watchdog: aborting after 150s');
  process.exit(1);
}, 150000);

let context = null;
try {
  // ---- 1. A vault with one known login -------------------------------
  const env = { ...process.env, UNBUNDIO_VAULT: vault, UNBUNDIO_VAULT_PASSWORD: MASTER_PW };
  execFileSync(BIN, ['--vault', vault, 'init'], { env, stdio: 'ignore' });
  execFileSync(
    BIN,
    ['--vault', vault, 'add', '--title', 'Example', '--username', 'alice',
     '--password', 'S3cret!Fill#2026', '--url', 'https://example.com',
     '--notes', 'e2e'],
    { env, stdio: 'ignore' },
  );

  // ---- 2. Native messaging manifest inside the throwaway profile -----
  mkdirSync(nmDir, { recursive: true });
  writeFileSync(
    join(nmDir, 'com.unbundio.vault.json'),
    JSON.stringify({
      name: 'com.unbundio.vault',
      description: 'unbundio-vault e2e',
      path: BIN,
      type: 'stdio',
      allowed_origins: [`chrome-extension://${extId}/`],
    }),
  );

  // ---- 3. Launch with the extension ----------------------------------
  context = await chromium.launchPersistentContext(profile, {
    // Bundled Chromium: Chrome 137+ ignores --load-extension from the CLI.
    // Real Chrome is loaded by hand (chrome://extensions -> Load unpacked).
    headless: false,
    args: [
      `--disable-extensions-except=${EXT_DIR}`,
      `--load-extension=${EXT_DIR}`,
      '--no-first-run',
      '--no-default-browser-check',
    ],
    env,
  });

  let sw = context.serviceWorkers()[0];
  if (!sw) sw = await context.waitForEvent('serviceworker', { timeout: 20000 });
  check('extension loaded (service worker running)', sw.url().includes(extId), sw.url());

  // ---- 4. Native Messaging, through the extension's own client -------
  const native = await sw.evaluate(async () => {
    const out = {};
    try {
      out.ping = await uvNativeCall({ action: 'ping' }, 8000);
    } catch (e) {
      out.pingErr = String((e && e.message) || e);
    }
    try {
      out.list = await uvNativeCall({ action: 'list' }, 8000);
      out.audit = await uvNativeCall({ action: 'audit' }, 8000);
      out.gen = await uvNativeCall({ action: 'gen', length: 24 }, 8000);
    } catch (e) {
      out.listErr = String((e && e.message) || e);
    }
    return out;
  });

  check('host answers ping()', !!native.ping && native.ping.host === 'com.unbundio.vault',
    native.pingErr || JSON.stringify(native.ping));
  check('host answers list()', Array.isArray(native.list) && native.list.length === 1,
    native.listErr || JSON.stringify(native.list));
  check('list() never carries passwords',
    !JSON.stringify(native.list || {}).includes('S3cret'));
  check('host answers audit()', native.audit && native.audit.total === 1,
    JSON.stringify(native.audit || {}));
  const gen = (native.gen && native.gen.password) || '';
  check('host generates a 24-char password', gen.length === 24, `${gen.length} chars`);

  // ---- 5. Content script fills a real login form ---------------------
  const page = await context.newPage();
  await page.setContent(`
    <form id="f">
      <input id="u" type="text" name="username">
      <input id="p" type="password" name="password">
      <button type="submit">Log in</button>
    </form>`);
  // Stub only the messaging port chrome.* provides to an injected script.
  await page.evaluate(() => {
    window.__uvListener = null;
    window.chrome = { runtime: { onMessage: { addListener: (fn) => { window.__uvListener = fn; } } } };
  });
  await page.addScriptTag({ content: readFileSync(join(EXT_DIR, 'content.js'), 'utf8') });
  const fillRes = await page.evaluate(
    (cred) =>
      new Promise((resolve) => {
        if (!window.__uvListener) return resolve({ ok: false, error: 'listener not registered' });
        window.__uvListener({ type: 'uv-fill', ...cred }, {}, resolve);
        setTimeout(() => resolve({ ok: false, error: 'no response' }), 5000);
      }),
    { username: 'alice', password: 'S3cret!Fill#2026' },
  );
  check('content script accepted the fill', fillRes && fillRes.ok === true, JSON.stringify(fillRes));

  const u = await page.inputValue('#u');
  const p = await page.inputValue('#p');
  check('username field filled from vault', u === 'alice', `got "${u}"`);
  check('password field filled from vault', p === 'S3cret!Fill#2026', p ? `got "${p}"` : '(empty)');

  // A page without a password field must fail loudly, not hang.
  const bare = await context.newPage();
  await bare.setContent('<input id="only" type="text">');
  await bare.evaluate(() => {
    window.__uvListener = null;
    window.chrome = { runtime: { onMessage: { addListener: (fn) => { window.__uvListener = fn; } } } };
  });
  await bare.addScriptTag({ content: readFileSync(join(EXT_DIR, 'content.js'), 'utf8') });
  const bareRes = await bare.evaluate(
    (cred) =>
      new Promise((resolve) => {
        window.__uvListener({ type: 'uv-fill', ...cred }, {}, resolve);
        setTimeout(() => resolve({ ok: false, error: 'no response' }), 5000);
      }),
    { username: 'x', password: 'y' },
  );
  check('refuses a page with no password field', bareRes && bareRes.ok === false,
    JSON.stringify(bareRes));

  // ---- 6. Popup UI renders ------------------------------------------
  const popup = await context.newPage();
  await popup.addInitScript(() => {
    // A page (not a popup) has no chrome.tabs; the listing path is what we check.
    // Mutate the built-in window.chrome — assigning window.chrome itself does not stick.
    if (!window.chrome) window.chrome = {};
    window.chrome.tabs = { query: async () => [] };
  });
  const popupHtml = readFileSync(join(EXT_DIR, 'popup.html'), 'utf8');
  await popup.setContent(popupHtml.replace(/<script src="[^"]+"><\/script>/g, ''));
  await popup.addScriptTag({ content: readFileSync(join(EXT_DIR, 'native.js'), 'utf8') });
  // Feed the popup real data straight from the host, bypassing chrome.* only.
  const listJson = JSON.stringify(native.list);
  const auditJson = JSON.stringify(native.audit);
  await popup.addScriptTag({
    content: `
      chrome.runtime = {
        connectNative: () => {
          const listeners = [];
          return {
            postMessage(msg) {
              const payload = msg.action === 'audit'
                ? ${auditJson}
                : (msg.action === 'list' ? ${listJson} : { id: msg.id, ok: true, result: {} });
              listeners.forEach((fn) => fn({ id: msg.id, ok: true, result: payload }));
            },
            onMessage: { addListener: (fn) => listeners.push(fn) },
            onDisconnect: { addListener: () => {} },
            disconnect() {},
          };
        },
      };`,
  });
  await popup.addScriptTag({ content: readFileSync(join(EXT_DIR, 'popup.js'), 'utf8') });
  // setContent already fired DOMContentLoaded, so boot() would never run.
  await popup.evaluate(() => document.dispatchEvent(new Event('DOMContentLoaded')));
  await popup.waitForSelector('.row .t', { timeout: 10000 }).catch(() => {});
  const rows = await popup.locator('.row').count();
  const title = await popup.locator('.row .t').first().textContent().catch(() => '');
  const auditText = await popup.locator('#audit').textContent();
  const popupStatus = await popup.locator('#status').textContent();
  console.log('      [popup status]', JSON.stringify(popupStatus));
  check('popup lists the entry', rows === 1, `${rows} row(s)`);
  check('popup shows the title', (title || '').includes('Example'), title || '');
  check('popup shows audit summary', /audit:/.test(auditText || ''), auditText || '(empty)');

  await popup.click('#genToggle');
  const genOut = await popup.locator('#genOut').textContent();
  check('popup generator produces a password', (genOut || '').length === 20,
    `${(genOut || '').length} chars`);
  await popup.close();
} catch (err) {
  console.error('harness error:', (err && err.message) || err);
  failures += 1;
} finally {
  clearTimeout(watchdog);
  if (context) await context.close().catch(() => {});
  try {
    rmSync(work, { recursive: true, force: true, maxRetries: 3 });
  } catch (_e) {
    /* Chrome may still hold the profile; the OS temp dir collects it */
  }
}

console.log(failures === 0 ? '\nALL CHECKS PASSED' : `\n${failures} CHECK(S) FAILED`);
console.log('Note: the toolbar click -> activeTab grant is verified manually (INSTALL.ko.md).');
process.exit(failures === 0 ? 0 : 1);