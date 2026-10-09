/* Content script (injected on demand via chrome.scripting, no persistent
 * host permissions): fills username + password fields. Original code. */
'use strict';

(function () {
  if (window.__uvInjected) return;
  window.__uvInjected = true;

  function visible(el) {
    if (!el || el.disabled || el.readOnly) return false;
    const r = el.getBoundingClientRect();
    if (!r || r.width === 0 || r.height === 0) return false;
    const style = window.getComputedStyle(el);
    return style.visibility !== 'hidden' && style.display !== 'none';
  }

  function setValue(el, value) {
    // Works with React/Vue controlled inputs: use the native setter.
    const proto = el.tagName === 'TEXTAREA'
      ? window.HTMLTextAreaElement.prototype
      : window.HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(proto, 'value').set;
    setter.call(el, value);
    el.dispatchEvent(new Event('input', { bubbles: true }));
    el.dispatchEvent(new Event('change', { bubbles: true }));
  }

  function passwordField() {
    const all = Array.from(document.querySelectorAll('input[type="password"]'));
    return all.find(visible) || null;
  }

  function usernameField(pw) {
    const scope = (pw && pw.form) || document;
    const candidates = Array.from(
      scope.querySelectorAll(
        'input[type="text"], input[type="email"], input:not([type])',
      ),
    ).filter(visible);
    if (!candidates.length) return null;
    if (!pw) return candidates[0];
    // Closest field above the password input in document order.
    let best = null;
    for (const c of candidates) {
      if (c === pw) continue;
      if (c.compareDocumentPosition(pw) & Node.DOCUMENT_POSITION_FOLLOWING) {
        best = c; // c comes before pw; keep the nearest one
      }
    }
    return best || candidates[0];
  }

  chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
    if (!msg || msg.type !== 'uv-fill') return false;
    (async () => {
      try {
        const pw = passwordField();
        if (!pw) {
          sendResponse({ ok: false, error: 'no password field on this page' });
          return;
        }
        const user = usernameField(pw);
        let n = 0;
        if (user && msg.username) {
          setValue(user, msg.username);
          n += 1;
        }
        setValue(pw, msg.password);
        n += 1;
        pw.focus();
        sendResponse({ ok: true, fields: n });
      } catch (e) {
        sendResponse({ ok: false, error: String((e && e.message) || e) });
      }
    })();
    return true; // async response
  });
})();
