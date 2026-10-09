/* Background service worker: thin relay so future features (context menu,
 * auto-suggest badges) have one place to live. Popup can also call the host
 * directly; the worker just re-exports the same actions via messages. */
'use strict';

importScripts('native.js');

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (!msg || msg.scope !== 'uv') return false;
  uvNativeCall({ action: msg.action, id: msg.id, query: msg.query })
    .then(
      (result) => sendResponse({ ok: true, result }),
      (err) => sendResponse({ ok: false, error: String((err && err.message) || err) }),
    );
  return true; // async response
});
