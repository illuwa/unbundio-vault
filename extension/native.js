/* Shared native-host client (loaded by background SW via importScripts
 * and by popup.html via <script>). Original code, no dependencies.
 * Speaks the unbundio-vault `host` protocol over Chrome Native Messaging:
 * request {id, action, ...} -> response {id, ok, result|error}. */
'use strict';

const UV_HOST = 'com.unbundio.vault';
let uvSeq = 0;

/** One request/response round-trip to the local vault host. */
function uvNativeCall(msg, timeoutMs) {
  const timeout = timeoutMs || 8000;
  return new Promise((resolve, reject) => {
    let port;
    try {
      port = chrome.runtime.connectNative(UV_HOST);
    } catch (e) {
      reject(new Error('vault host not installed (run: unbundio-vault install-extension)'));
      return;
    }
    const id = ++uvSeq;
    let done = false;
    const finish = (fn, val) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      try { port.disconnect(); } catch (_e) { /* ignore */ }
      fn(val);
    };
    const timer = setTimeout(
      () => finish(reject, new Error('vault host timeout')),
      timeout,
    );
    port.onMessage.addListener((resp) => {
      if (!resp || resp.id !== id) return;
      if (resp.ok) finish(resolve, resp.result);
      else finish(reject, new Error(resp.error || 'vault host error'));
    });
    port.onDisconnect.addListener(() => {
      const err = chrome.runtime.lastError && chrome.runtime.lastError.message;
      finish(reject, new Error(err || 'vault host disconnected'));
    });
    try {
      const req = Object.assign({ id }, msg);
      port.postMessage(req);
    } catch (e) {
      finish(reject, e);
    }
  });
}
