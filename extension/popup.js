/* Popup: search the local vault, fill/copy in one click, generate passwords.
 * Original code, no dependencies. Talks to the vault host over Native
 * Messaging (see native.js) — no server, no account. */
'use strict';

const $ = (id) => document.getElementById(id);

let entries = [];
let audit = null;
let selected = 0;
let generatorOn = false;

function setStatus(text, isErr) {
  const el = $('status');
  el.textContent = text || '';
  el.classList.toggle('err', !!isErr);
}

function hostOf(url) {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch (_e) {
    return '';
  }
}

function activeTab() {
  return chrome.tabs.query({ active: true, currentWindow: true }).then((t) => t[0]);
}

/** Same filter rule as the Rust side: id prefix, title, username or url. */
function filtered(q) {
  const needle = q.trim().toLowerCase();
  if (!needle) return entries;
  return entries.filter(
    (e) =>
      e.title.toLowerCase().includes(needle) ||
      e.username.toLowerCase().includes(needle) ||
      (e.url || '').toLowerCase().includes(needle),
  );
}

function visibleRows() {
  return filtered($('q').value).slice(0, 50);
}

function render() {
  const list = $('list');
  list.textContent = '';
  const rows = visibleRows();
  $('count').textContent = entries.length ? `${entries.length}` : '';

  if (!rows.length) {
    const d = document.createElement('div');
    d.className = 'empty';
    d.textContent = entries.length
      ? 'No matches.'
      : 'Vault is empty — add entries with: unbundio-vault add';
    list.appendChild(d);
    return;
  }

  rows.forEach((e, i) => {
    const row = document.createElement('div');
    row.className = 'row' + (i === selected ? ' sel' : '');
    row.dataset.id = e.id;

    const meta = document.createElement('div');
    meta.className = 'meta';
    const t = document.createElement('div');
    t.className = 't';
    // textContent, never innerHTML: entry text is untrusted input.
    t.textContent = (e.favorite ? '★ ' : '') + e.title;
    const u = document.createElement('div');
    u.className = 'u';
    u.textContent = e.username;
    meta.appendChild(t);
    meta.appendChild(u);

    const fill = document.createElement('button');
    fill.className = 'primary';
    fill.textContent = 'Fill';
    fill.addEventListener('click', () => fillEntry(e));

    const copy = document.createElement('button');
    copy.textContent = 'Copy';
    copy.title = 'Copy password';
    copy.addEventListener('click', () => copyEntry(e));

    row.appendChild(meta);
    row.appendChild(fill);
    row.appendChild(copy);
    row.addEventListener('mouseenter', () => {
      selected = i;
      markSelection();
    });
    list.appendChild(row);
  });
}

function markSelection() {
  const rows = $('list').querySelectorAll('.row');
  rows.forEach((r, i) => r.classList.toggle('sel', i === selected));
  const sel = rows[selected];
  if (sel && sel.scrollIntoView) sel.scrollIntoView({ block: 'nearest' });
}

async function fillEntry(e) {
  setStatus('');
  try {
    const tab = await activeTab();
    if (!tab || tab.id === undefined) throw new Error('no active tab');
    const full = await uvNativeCall({ action: 'get', id: e.id });
    await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      files: ['content.js'],
    });
    const res = await chrome.tabs.sendMessage(tab.id, {
      type: 'uv-fill',
      username: full.username,
      password: full.password,
    });
    if (res && res.ok) {
      setStatus(`Filled ${res.fields} field(s).`);
      window.close();
    } else {
      throw new Error((res && res.error) || 'page did not accept fill');
    }
  } catch (err) {
    setStatus(`Fill failed: ${err.message}`, true);
  }
}

async function copyEntry(e) {
  setStatus('');
  try {
    const full = await uvNativeCall({ action: 'get', id: e.id });
    await copyText(full.password);
    setStatus('Password copied (30s).');
    setTimeout(() => navigator.clipboard.writeText('').catch(() => {}), 30000);
  } catch (err) {
    setStatus(`Copy failed: ${err.message}`, true);
  }
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch (_e) {
    /* fall through for pages/popups without clipboard permission */
  }
  const ta = document.createElement('textarea');
  ta.value = text;
  document.body.appendChild(ta);
  ta.select();
  document.execCommand('copy');
  ta.remove();
}

function newPassword() {
  // Local entropy from the Web Crypto CSPRNG — no network, no host round-trip.
  const sets = [
    'abcdefghijkmnopqrstuvwxyz',
    'ABCDEFGHJKLMNPQRSTUVWXYZ',
    '23456789',
    '!@#$%^&*-_=+?',
  ];
  const alphabet = sets.join('');
  const bytes = new Uint8Array(20);
  crypto.getRandomValues(bytes);
  const chars = sets.map((s) => s[bytes[sets.indexOf(s)] % s.length]);
  let out = '';
  for (let i = 0; i < 20; i += 1) {
    out += alphabet[bytes[i % bytes.length] % alphabet.length];
  }
  // Guarantee one from each class.
  let k = 0;
  for (const c of chars) out = out.slice(0, k) + c + out.slice(k + 1), (k += 5);
  return out;
}

function renderGenerator() {
  $('gen').classList.toggle('open', generatorOn);
  $('genOut').textContent = generatorOn ? newPassword() : '';
}

function auditSummary() {
  if (!audit) return '';
  const bits = [];
  if (audit.weak && audit.weak.length) bits.push(`${audit.weak.length} weak`);
  if (audit.reused && audit.reused.length) bits.push(`${audit.reused.length} reused`);
  if (!bits.length) return 'audit: clean';
  return 'audit: ' + bits.join(' · ');
}

function onKey(ev) {
  const rows = visibleRows();
  if (ev.key === 'ArrowDown') {
    ev.preventDefault();
    selected = Math.min(selected + 1, rows.length - 1);
    markSelection();
  } else if (ev.key === 'ArrowUp') {
    ev.preventDefault();
    selected = Math.max(selected - 1, 0);
    markSelection();
  } else if (ev.key === 'Enter') {
    ev.preventDefault();
    const e = rows[selected];
    if (e) fillEntry(e);
  } else if (ev.key === 'Escape') {
    if (generatorOn) toggleGenerator(false);
    else window.close();
  }
}

function toggleGenerator(force) {
  generatorOn = force === undefined ? !generatorOn : force;
  renderGenerator();
  setStatus('');
}

async function boot() {
  try {
    entries = await uvNativeCall({ action: 'list' });
  } catch (err) {
    setStatus(
      `Vault unreachable: ${err.message}. Is the host installed?`,
      true,
    );
    entries = [];
  }
  // Security posture, computed by the same local rules the CLI audit uses.
  try {
    const all = await uvNativeCall({ action: 'audit' });
    audit = all;
    $('audit').textContent = auditSummary();
  } catch (_e) {
    $('audit').textContent = '';
  }

  try {
    const tab = await activeTab();
    const h = tab && tab.url ? hostOf(tab.url) : '';
    if (h) {
      const matches = entries.filter((e) =>
        (e.url || '').toLowerCase().includes(h.toLowerCase()),
      );
      if (matches.length) {
        entries = matches.concat(entries.filter((e) => matches.indexOf(e) === -1));
        $('q').value = h;
        selected = 0;
      }
    }
  } catch (_e) {
    /* no tab info: show everything */
  }

  render();
  $('q').addEventListener('input', () => {
    selected = 0;
    render();
  });
  $('q').addEventListener('keydown', onKey);
  $('q').focus();
  $('genToggle').addEventListener('click', () => toggleGenerator());
  $('genRegen').addEventListener('click', renderGenerator);
  $('genCopy').addEventListener('click', async () => {
    await copyText($('genOut').textContent);
    setStatus('Generated password copied.');
  });
}

document.addEventListener('DOMContentLoaded', boot);