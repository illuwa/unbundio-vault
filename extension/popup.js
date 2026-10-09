/* Popup: search local vault, one-click fill or copy. Original code. */
'use strict';

const $ = (id) => document.getElementById(id);
let entries = [];

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

async function activeTab() {
  const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
  return tabs[0];
}

function filtered(q) {
  const needle = q.trim().toLowerCase();
  if (!needle) return entries;
  return entries.filter((e) =>
    e.title.toLowerCase().includes(needle) ||
    e.username.toLowerCase().includes(needle) ||
    (e.url || '').toLowerCase().includes(needle),
  );
}

function render(q) {
  const list = $('list');
  list.textContent = '';
  const rows = filtered(q).slice(0, 50);
  if (!rows.length) {
    const d = document.createElement('div');
    d.className = 'row';
    d.textContent = entries.length ? 'No matches.' : 'Vault is empty.';
    list.appendChild(d);
    return;
  }
  for (const e of rows) {
    const row = document.createElement('div');
    row.className = 'row';
    const meta = document.createElement('div');
    meta.className = 'meta';
    const t = document.createElement('div');
    t.className = 't';
    t.textContent = (e.favorite ? '★ ' : '') + e.title;
    const u = document.createElement('div');
    u.className = 'u';
    u.textContent = e.username;
    meta.appendChild(t);
    meta.appendChild(u);
    const fill = document.createElement('button');
    fill.textContent = 'Fill';
    fill.addEventListener('click', () => fillEntry(e));
    const copy = document.createElement('button');
    copy.textContent = 'Copy';
    copy.title = 'Copy password';
    copy.addEventListener('click', () => copyEntry(e));
    row.appendChild(meta);
    row.appendChild(fill);
    row.appendChild(copy);
    list.appendChild(row);
  }
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
    if (res && res.ok) setStatus(`Filled ${res.fields} field(s).`);
    else throw new Error((res && res.error) || 'page did not accept fill');
  } catch (err) {
    setStatus(`Fill failed: ${err.message}`, true);
  }
}

async function copyEntry(e) {
  setStatus('');
  try {
    const full = await uvNativeCall({ action: 'get', id: e.id });
    await copyText(full.password);
    setStatus('Password copied.');
  } catch (err) {
    setStatus(`Copy failed: ${err.message}`, true);
  }
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch (_e) { /* fall through to legacy path */ }
  const ta = document.createElement('textarea');
  ta.value = text;
  document.body.appendChild(ta);
  ta.select();
  document.execCommand('copy');
  ta.remove();
}

async function boot() {
  try {
    entries = await uvNativeCall({ action: 'list' });
  } catch (err) {
    setStatus(`Vault unreachable: ${err.message}. Is the host installed?`, true);
    entries = [];
  }
  // Preselect by current site so the right login is usually on top.
  try {
    const tab = await activeTab();
    const h = tab && tab.url ? hostOf(tab.url) : '';
    if (h) {
      const matches = entries.filter((e) =>
        (e.url || '').toLowerCase().includes(h.toLowerCase()),
      );
      if (matches.length) {
        entries = matches.concat(entries.filter((e) => !matches.includes(e)));
        $('q').value = h;
      }
    }
  } catch (_e) { /* no active tab info; show all */ }
  render($('q').value);
  $('q').addEventListener('input', (ev) => render(ev.target.value));
  $('q').focus();
}

document.addEventListener('DOMContentLoaded', boot);
