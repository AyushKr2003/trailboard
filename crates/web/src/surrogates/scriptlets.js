// Scriptlets: small functions a filter rule runs in a page before the page's
// own scripts (`example.com##+js(name, arg, …)`), mostly to get past
// "turn off your ad blocker" walls. Our own implementations of the
// behaviour the common filter lists expect from each name; `web::resources`
// reads the sections. A `//@` line names a resource and its aliases (`.js`
// is added to a name without it); a `+name` token is a helper it needs.
// Helpers (`.fn`) are plain function declarations shared by several
// scriptlets. Everything is injected inside one function, so none of it is
// visible to the page.

//@ tb-util.fn
// A filter argument as a test on text: `/re/flags` is a regular
// expression, anything else a plain substring; '' matches everything. A
// leading `!` negates the test.
function tbMatcher(raw) {
  let s = raw === undefined || raw === null ? '' : String(raw);
  let not = false;
  if (s.startsWith('!')) { not = true; s = s.slice(1); }
  let re;
  const m = /^\/(.+)\/([imsu]*)$/.exec(s);
  if (m) {
    try { re = new RegExp(m[1], m[2]); } catch (_) { re = /(?!)/; }
  } else {
    re = new RegExp(s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  }
  return { empty: s === '', test: (text) => re.test(String(text)) !== not };
}
// The value a rule may set: a short list of harmless constants, as the lists
// expect them. Unknown values come back as `tbValue.bad`.
function tbValue(raw) {
  switch (raw) {
    case 'undefined': return undefined;
    case 'false': return false;
    case 'true': return true;
    case 'null': return null;
    case "''": case '""': case '': case 'emptyStr': return '';
    case 'noopFunc': return function() {};
    case 'trueFunc': return function() { return true; };
    case 'falseFunc': return function() { return false; };
    case 'throwFunc': return function() { throw new Error(); };
    case 'emptyObj': case '{}': return {};
    case 'emptyArr': case '[]': return [];
    case 'noopCallbackFunc': return function() { return function() {}; };
    case 'NaN': return NaN;
    case 'Infinity': return Infinity;
    case '-Infinity': return -Infinity;
    case 'yes': case 'no': case 'on': case 'off': case 'accept': case 'reject':
    case 'accepted': case 'rejected': case 'allow': case 'deny': case 'ok': case 'necessary':
      return raw;
  }
  if (/^-?\d+(\.\d+)?$/.test(raw)) {
    const n = Number(raw);
    if (Math.abs(n) <= 0x7fff) return n;
  }
  return tbValue.bad;
}
tbValue.bad = Symbol('bad value');
// The text of a function or handler object, to match a needle against.
function tbFnText(f) {
  try {
    if (typeof f === 'function') return Function.prototype.toString.call(f);
    if (f && typeof f.handleEvent === 'function') return Function.prototype.toString.call(f.handleEvent);
    return String(f);
  } catch (_) { return ''; }
}
// Replaces `owner[name]` (a function) with a proxy whose calls go to
// `handler(target, thisArg, args)`; `toString` still shows the original.
function tbWrap(owner, name, handler) {
  const target = owner && owner[name];
  if (typeof target !== 'function') return;
  try {
    owner[name] = new Proxy(target, { apply: (t, self, args) => handler(t, self, args) });
  } catch (_) {}
}
// An error to stop a page script with, kept out of the page's error
// reports.
function tbAbort() {
  if (!tbAbort.msg) {
    tbAbort.msg = 'tb' + Math.random().toString(36).slice(2);
    window.addEventListener('error', (e) => {
      if (typeof e.message === 'string' && e.message.includes(tbAbort.msg)) e.preventDefault();
    }, true);
  }
  return new ReferenceError(tbAbort.msg);
}
// Calls `found(owner, prop)` once `chain` (`a.b.c`) reaches its last
// object, waiting on the properties along the way if they don't exist yet.
function tbChain(owner, chain, found) {
  const dot = chain.indexOf('.');
  if (dot === -1) { found(owner, chain); return; }
  const prop = chain.slice(0, dot);
  const rest = chain.slice(dot + 1);
  const v = owner[prop];
  if (v !== null && (typeof v === 'object' || typeof v === 'function')) { tbChain(v, rest, found); return; }
  const desc = Object.getOwnPropertyDescriptor(owner, prop);
  if (desc && (desc.configurable === false || desc.get || desc.set)) return;
  let current = v;
  try {
    Object.defineProperty(owner, prop, {
      configurable: true,
      enumerable: true,
      get() { return current; },
      set(a) {
        current = a;
        if (a !== null && (typeof a === 'object' || typeof a === 'function')) tbChain(a, rest, found);
      },
    });
  } catch (_) {}
}
// Whether a request (`url`, plus `init` fields) matches a list of
// `key:value` conditions (a bare value is the URL); '' is no condition.
function tbRequestMatches(conds, url, init) {
  const parts = String(conds || '').trim().split(/\s+/).filter(Boolean);
  if (!parts.length) return false;
  for (const p of parts) {
    const c = p.indexOf(':');
    let key = 'url', val = p;
    if (c > 0 && /^[a-zA-Z]+$/.test(p.slice(0, c)) && !/^https?$/i.test(p.slice(0, c))) { key = p.slice(0, c); val = p.slice(c + 1); }
    const have = key === 'url' ? url : init && init[key] !== undefined ? init[key] : '';
    if (!tbMatcher(val).test(typeof have === 'string' ? have : String(have))) return false;
  }
  return true;
}
// Deletes the paths in `prune` (space-separated, `a.b`, `[]` for every
// array element, `*` for every key) from `obj` if all of `needles` exist.
function tbPrune(obj, prune, needles) {
  if (obj === null || typeof obj !== 'object') return obj;
  const paths = (s) => String(s || '').split(/\s+/).filter(Boolean);
  const walk = (o, parts, act) => {
    if (o === null || typeof o !== 'object') return false;
    const [head, ...rest] = parts;
    const keys = head === '[]' || head === '*' ? Object.keys(o) : [head];
    let hit = false;
    for (const k of keys) {
      if (!Object.prototype.hasOwnProperty.call(o, k)) continue;
      if (rest.length === 0) { hit = true; if (act) delete o[k]; } else if (walk(o[k], rest, act)) hit = true;
    }
    return hit;
  };
  const split = (p) => p.replace(/\[\]/g, '.[]').split('.').filter(Boolean);
  for (const n of paths(needles)) if (!walk(obj, split(n), false)) return obj;
  for (const p of paths(prune)) walk(obj, split(p), true);
  return obj;
}
// Runs `fn` now and after every change to the document, until the page has
// loaded (or for good, with `stay`).
function tbOnDom(fn, stay) {
  const run = () => { try { fn(); } catch (_) {} };
  let pending = false;
  const obs = new MutationObserver(() => {
    if (pending) return;
    pending = true;
    queueMicrotask(() => { pending = false; run(); });
  });
  const start = () => {
    run();
    obs.observe(document, { childList: true, subtree: true, attributes: true });
    if (!stay) window.addEventListener('load', () => setTimeout(() => obs.disconnect(), 1000), { once: true });
  };
  if (document.documentElement) start(); else document.addEventListener('readystatechange', start, { once: true });
}

//@ set-constant set +tb-util.fn
function tbSetConstant(chain = '', raw = '', ...extra) {
  if (!chain) return;
  let value = tbValue(raw);
  if (value === tbValue.bad) return;
  if (extra.includes('asFunction')) { const v = value; value = function() { return v; }; }
  else if (extra.includes('asCallback')) { const v = value; value = function() { return function() { return v; }; }; }
  else if (extra.includes('asResolved')) value = Promise.resolve(value);
  else if (extra.includes('asRejected')) { value = Promise.reject(value); value.catch(() => {}); }
  // A page value of another type is left alone (setting it would break more than it fixes).
  const clash = (v) => v !== undefined && v !== null && value !== undefined && value !== null && typeof v !== typeof value;
  tbChain(window, chain.replace(/^window\./, ''), (owner, prop) => {
    const desc = Object.getOwnPropertyDescriptor(owner, prop);
    if (desc && desc.configurable === false) return;
    if (desc && 'value' in desc && clash(desc.value)) return;
    let own = value;
    try {
      Object.defineProperty(owner, prop, {
        configurable: false,
        enumerable: desc ? desc.enumerable : true,
        get() { return own; },
        set(v) { if (clash(v)) own = v; },
      });
    } catch (_) {}
  });
}

//@ abort-on-property-read aopr +tb-util.fn
function tbAbortOnRead(chain = '') {
  if (!chain) return;
  tbChain(window, chain.replace(/^window\./, ''), (owner, prop) => {
    try {
      Object.defineProperty(owner, prop, {
        configurable: false,
        get() { throw tbAbort(); },
        set() {},
      });
    } catch (_) {}
  });
}

//@ abort-on-property-write aopw +tb-util.fn
function tbAbortOnWrite(chain = '') {
  if (!chain) return;
  tbChain(window, chain.replace(/^window\./, ''), (owner, prop) => {
    try {
      delete owner[prop];
      Object.defineProperty(owner, prop, {
        configurable: false,
        get() { return undefined; },
        set() { throw tbAbort(); },
      });
    } catch (_) {}
  });
}

//@ abort-current-script acs abort-current-inline-script acis +tb-util.fn
function tbAbortCurrentScript(chain = '', needle = '', context = '') {
  if (!chain) return;
  const match = tbMatcher(needle);
  const ctx = tbMatcher(context);
  const thisScript = document.currentScript;
  const stop = () => {
    const s = document.currentScript;
    if (!s || s === thisScript || !(s instanceof HTMLScriptElement)) return;
    const text = s.src ? s.src + '\n' + s.textContent : s.textContent;
    if (!ctx.empty && !ctx.test(text)) return;
    if (match.test(text)) throw tbAbort();
  };
  tbChain(window, chain.replace(/^window\./, ''), (owner, prop) => {
    const desc = Object.getOwnPropertyDescriptor(owner, prop);
    if (desc && desc.configurable === false) return;
    let value = desc ? (desc.get ? undefined : desc.value) : undefined;
    const get = desc && desc.get;
    const set = desc && desc.set;
    try {
      Object.defineProperty(owner, prop, {
        configurable: true,
        enumerable: desc ? desc.enumerable : true,
        get() { stop(); return get ? get.call(this) : value; },
        set(v) { stop(); if (set) set.call(this, v); else value = v; },
      });
    } catch (_) {}
  });
}

//@ abort-on-stack-trace aost +tb-util.fn
function tbAbortOnStackTrace(chain = '', needle = '') {
  if (!chain) return;
  const match = tbMatcher(needle);
  const page = location.href.split('#')[0];
  const stack = () => {
    const lines = String(new Error().stack || '').split('\n').slice(2);
    return lines.map((l) => {
      const m = /(https?:\/\/[^\s)]+?)(:\d+:\d+)?\)?$/.exec(l.trim());
      if (!m) return l.trim();
      return m[1].split('#')[0] === page ? 'inlineScript' : m[1];
    }).join('\n');
  };
  tbChain(window, chain.replace(/^window\./, ''), (owner, prop) => {
    const desc = Object.getOwnPropertyDescriptor(owner, prop);
    if (desc && desc.configurable === false) return;
    let value = desc ? desc.value : undefined;
    try {
      Object.defineProperty(owner, prop, {
        configurable: true,
        get() { if (match.test(stack())) throw tbAbort(); return desc && desc.get ? desc.get.call(this) : value; },
        set(v) { if (match.test(stack())) throw tbAbort(); if (desc && desc.set) desc.set.call(this, v); else value = v; },
      });
    } catch (_) {}
  });
}

//@ tb-timer.fn
// Wraps a timer function so callbacks matching `needle` (and `delay`: '' any,
// `N` exactly, `!N` anything but) are dropped, or their delay changed.
function tbTimer(name, needle, delay, change) {
  const match = tbMatcher(needle);
  let d = String(delay === undefined ? '' : delay);
  const notDelay = d.startsWith('!');
  if (notDelay) d = d.slice(1);
  const want = d === '' ? null : parseInt(d, 10);
  tbWrap(window, name, (t, self, args) => {
    const [fn, ms] = args;
    const delayOk = want === null || Number.isNaN(want) || ((ms | 0) === want) !== notDelay;
    if (delayOk && match.test(tbFnText(fn))) {
      if (change) args[1] = change(ms | 0);
      else args[0] = function() {};
    }
    return Reflect.apply(t, self, args);
  });
}

//@ no-setTimeout-if nostif prevent-setTimeout setTimeout-defuser +tb-util.fn +tb-timer.fn
function tbNoSetTimeoutIf(needle = '', delay = '') {
  if (needle === '' && delay === '') return;
  tbTimer('setTimeout', needle, delay);
}

//@ no-setInterval-if nosiif prevent-setInterval setInterval-defuser +tb-util.fn +tb-timer.fn
function tbNoSetIntervalIf(needle = '', delay = '') {
  if (needle === '' && delay === '') return;
  tbTimer('setInterval', needle, delay);
}

//@ no-requestAnimationFrame-if norafif prevent-requestAnimationFrame +tb-util.fn
function tbNoRafIf(needle = '') {
  const match = tbMatcher(needle);
  if (match.empty) return;
  tbWrap(window, 'requestAnimationFrame', (t, self, args) => {
    if (match.test(tbFnText(args[0]))) args[0] = function() {};
    return Reflect.apply(t, self, args);
  });
}

//@ nano-setTimeout-booster nano-stb +tb-util.fn +tb-timer.fn
function tbNanoSetTimeout(needle = '', delay = '1000', boost = '0.05') {
  let b = parseFloat(boost);
  if (!(b >= 0.001 && b <= 50)) b = 0.05;
  tbTimer('setTimeout', needle, delay === '*' ? '' : delay || '1000', (ms) => ms * b);
}

//@ nano-setInterval-booster nano-sib +tb-util.fn +tb-timer.fn
function tbNanoSetInterval(needle = '', delay = '1000', boost = '0.05') {
  let b = parseFloat(boost);
  if (!(b >= 0.001 && b <= 50)) b = 0.05;
  tbTimer('setInterval', needle, delay === '*' ? '' : delay || '1000', (ms) => ms * b);
}

//@ prevent-addEventListener aeld addEventListener-defuser +tb-util.fn
function tbPreventListener(type = '', handler = '') {
  const t = tbMatcher(type);
  const h = tbMatcher(handler);
  if (t.empty && h.empty) return;
  tbWrap(EventTarget.prototype, 'addEventListener', (fn, self, args) => {
    if (t.test(args[0]) && h.test(tbFnText(args[1]))) return undefined;
    return Reflect.apply(fn, self, args);
  });
}

//@ no-window-open-if nowoif prevent-window-open window.open-defuser +tb-util.fn
function tbNoWindowOpenIf(needle = '') {
  const match = tbMatcher(needle);
  const noop = function() {};
  tbWrap(window, 'open', (t, self, args) => {
    if (!match.test(String(args[0] === undefined ? '' : args[0]))) return Reflect.apply(t, self, args);
    // A stand-in for the window, enough for scripts that poke at it.
    const fake = { closed: false, opener: window, document: document.implementation.createHTMLDocument(''),
      location: { href: '', assign: noop, replace: noop }, close() { this.closed = true; }, focus: noop, blur: noop,
      postMessage: noop, addEventListener: noop, removeEventListener: noop };
    return new Proxy(fake, { get: (o, k) => (k in o ? o[k] : noop) });
  });
}

//@ remove-node-text rmnt +tb-util.fn
function tbRemoveNodeText(nodeName = '', condition = '') {
  if (!nodeName) return;
  const name = tbMatcher(nodeName.startsWith('/') ? nodeName : '/^' + nodeName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '$/i');
  const cond = tbMatcher(condition);
  const check = (node) => {
    if (!name.test(node.nodeName)) return;
    const text = node.textContent;
    if (text && cond.test(text)) node.textContent = '';
  };
  const visit = (node) => {
    check(node);
    if (node.nodeType === 1 && node.firstChild) {
      const walker = document.createTreeWalker(node, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT);
      while (walker.nextNode()) check(walker.currentNode);
    }
  };
  const obs = new MutationObserver((records) => {
    for (const r of records) for (const n of r.addedNodes) visit(n);
  });
  const start = () => {
    if (document.documentElement) visit(document.documentElement);
    obs.observe(document, { childList: true, subtree: true });
  };
  if (document.documentElement) start(); else document.addEventListener('readystatechange', start, { once: true });
  window.addEventListener('load', () => setTimeout(() => obs.disconnect(), 3000), { once: true });
}

//@ no-fetch-if prevent-fetch +tb-util.fn
function tbNoFetchIf(conds = '', body = '') {
  if (!conds || typeof window.fetch !== 'function') return;
  const text = body === 'emptyObj' ? '{}' : body === 'emptyArr' ? '[]' : '';
  tbWrap(window, 'fetch', (t, self, args) => {
    const [input, init] = args;
    let url = '', fields = Object.assign({}, init);
    if (input instanceof Request) {
      url = input.url;
      for (const k of ['method', 'mode', 'credentials', 'cache', 'redirect', 'referrer', 'integrity']) {
        if (fields[k] === undefined) fields[k] = input[k];
      }
    } else url = String(input);
    if (!tbRequestMatches(conds, url, fields)) return Reflect.apply(t, self, args);
    const resp = new Response(text, { status: 200, statusText: 'OK', headers: { 'Content-Type': 'text/plain' } });
    try { Object.defineProperty(resp, 'url', { value: url }); } catch (_) {}
    if (fields.mode === 'no-cors') {
      try { Object.defineProperty(resp, 'type', { value: 'opaque' }); } catch (_) {}
    }
    return Promise.resolve(resp);
  });
}

//@ no-xhr-if prevent-xhr +tb-util.fn
function tbNoXhrIf(conds = '', body = '') {
  if (!conds) return;
  const proto = XMLHttpRequest.prototype;
  const asked = new WeakMap();
  tbWrap(proto, 'open', (t, self, args) => {
    asked.set(self, { method: String(args[0] || 'GET'), url: String(args[1] || '') });
    return Reflect.apply(t, self, args);
  });
  tbWrap(proto, 'send', (t, self, args) => {
    const req = asked.get(self);
    if (!req || !tbRequestMatches(conds, req.url, req)) return Reflect.apply(t, self, args);
    const text = body === 'emptyObj' ? '{}' : body === 'emptyArr' ? '[]' : '';
    let response = text;
    if (self.responseType === 'json') { try { response = JSON.parse(text || 'null'); } catch (_) { response = null; } }
    else if (self.responseType === 'arraybuffer') response = new ArrayBuffer(0);
    else if (self.responseType === 'blob') response = new Blob([]);
    else if (self.responseType === 'document') response = null;
    const set = (k, v) => { try { Object.defineProperty(self, k, { value: v, configurable: true }); } catch (_) {} };
    setTimeout(() => {
      set('readyState', 4); set('status', 200); set('statusText', 'OK'); set('responseURL', req.url);
      set('response', response);
      if (self.responseType === '' || self.responseType === 'text') set('responseText', text);
      for (const ev of ['readystatechange', 'load', 'loadend']) self.dispatchEvent(new Event(ev));
    }, 1);
    return undefined;
  });
}

//@ noeval-if prevent-eval-if +tb-util.fn
function tbNoEvalIf(needle = '') {
  const match = tbMatcher(needle);
  tbWrap(window, 'eval', (t, self, args) => {
    if (match.test(String(args[0]))) return undefined;
    return Reflect.apply(t, self, args);
  });
}

//@ noeval noeval-silent silent-noeval +tb-util.fn
function tbNoEval() {
  tbWrap(window, 'eval', () => undefined);
}

//@ json-prune +tb-util.fn
function tbJsonPrune(prune = '', needles = '') {
  if (!prune) return;
  tbWrap(JSON, 'parse', (t, self, args) => tbPrune(Reflect.apply(t, self, args), prune, needles));
  if (window.Response) {
    tbWrap(Response.prototype, 'json', (t, self, args) =>
      Reflect.apply(t, self, args).then((o) => tbPrune(o, prune, needles)));
  }
}

//@ json-prune-fetch-response +tb-util.fn
function tbJsonPruneFetch(prune = '', needles = '', ...extra) {
  if (!prune || typeof window.fetch !== 'function') return;
  const i = extra.indexOf('propsToMatch');
  const conds = i !== -1 ? extra[i + 1] : '';
  const parse = JSON.parse;
  tbWrap(window, 'fetch', (t, self, args) => {
    const url = args[0] instanceof Request ? args[0].url : String(args[0]);
    const p = Reflect.apply(t, self, args);
    if (conds && !tbRequestMatches(conds, url, args[1])) return p;
    return p.then((resp) => resp.clone().text().then((text) => {
      let obj;
      try { obj = parse(text); } catch (_) { return resp; }
      const out = new Response(JSON.stringify(tbPrune(obj, prune, needles)),
        { status: resp.status, statusText: resp.statusText, headers: resp.headers });
      try { Object.defineProperty(out, 'url', { value: resp.url }); } catch (_) {}
      return out;
    }).catch(() => resp));
  });
}

//@ remove-attr ra +tb-util.fn
function tbRemoveAttr(attrs = '', selector = '', behavior = '') {
  const names = attrs.split(/\s*\|\s*/).filter(Boolean);
  if (!names.length) return;
  const sel = selector || names.map((n) => `[${CSS.escape(n)}]`).join(',');
  tbOnDom(() => {
    for (const e of document.querySelectorAll(sel)) for (const n of names) if (e.hasAttribute(n)) e.removeAttribute(n);
  }, behavior.includes('stay'));
}

//@ remove-class rc +tb-util.fn
function tbRemoveClass(classes = '', selector = '', behavior = '') {
  const names = classes.split(/\s*\|\s*/).filter(Boolean);
  if (!names.length) return;
  const sel = selector || names.map((n) => '.' + CSS.escape(n)).join(',');
  tbOnDom(() => {
    for (const e of document.querySelectorAll(sel)) for (const n of names) if (e.classList.contains(n)) e.classList.remove(n);
  }, behavior.includes('stay'));
}

//@ set-attr +tb-util.fn
function tbSetAttr(selector = '', attr = '', value = '') {
  if (!selector || !attr) return;
  const copy = /^\[([\w-]+)\]$/.exec(value);
  if (!copy && !/^(|true|false|-?\d{1,5})$/.test(value)) return;
  tbOnDom(() => {
    for (const e of document.querySelectorAll(selector)) {
      const v = copy ? e.getAttribute(copy[1]) : value;
      if (v !== null && e.getAttribute(attr) !== v) e.setAttribute(attr, v);
    }
  }, true);
}

//@ tb-storage.fn
function tbSetStorage(store, key, raw) {
  if (!key) return;
  let s;
  try { s = window[store]; } catch (_) { return; }
  if (!s) return;
  if (raw === '$remove$') { try { s.removeItem(key); } catch (_) {} return; }
  const v = tbValue(raw);
  if (v === tbValue.bad || typeof v === 'function') return;
  const text = Array.isArray(v) ? '[]' : v !== null && typeof v === 'object' ? '{}' : String(v);
  try { if (s.getItem(key) !== text) s.setItem(key, text); } catch (_) {}
}

//@ set-local-storage-item +tb-util.fn +tb-storage.fn
function tbSetLocalStorage(key = '', value = '') {
  tbSetStorage('localStorage', key, value);
}

//@ set-session-storage-item +tb-util.fn +tb-storage.fn
function tbSetSessionStorage(key = '', value = '') {
  tbSetStorage('sessionStorage', key, value);
}

//@ set-cookie +tb-util.fn
function tbSetCookie(name = '', raw = '', path = '') {
  if (!name || /[;=\s]/.test(name)) return;
  const v = tbValue(raw);
  if (v === tbValue.bad || typeof v === 'function' || (v !== null && typeof v === 'object')) return;
  const value = String(v);
  if (/[;\s]/.test(value)) return;
  const have = document.cookie.split(/;\s*/).some((c) => c === `${encodeURIComponent(name)}=${value}`);
  if (have) return;
  const p = path === 'none' ? '' : `; path=${path || '/'}`;
  try { document.cookie = `${encodeURIComponent(name)}=${value}${p}; max-age=31536000; SameSite=Lax`; } catch (_) {}
}

//@ remove-cookie cookie-remover +tb-util.fn
function tbRemoveCookie(needle = '') {
  const match = tbMatcher(needle);
  const clear = () => {
    for (const c of document.cookie.split(';')) {
      const name = c.split('=')[0].trim();
      if (!name || !match.test(name)) continue;
      const host = location.hostname;
      const domains = ['', host, '.' + host];
      const parts = host.split('.');
      if (parts.length > 2) domains.push('.' + parts.slice(-2).join('.'));
      for (const d of domains) {
        for (const p of ['/', location.pathname]) {
          document.cookie = `${name}=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=${p}${d ? '; domain=' + d : ''}`;
        }
      }
    }
  };
  clear();
  window.addEventListener('beforeunload', clear);
  document.addEventListener('visibilitychange', clear);
}

//@ href-sanitizer +tb-util.fn
function tbHrefSanitizer(selector = '', source = 'text') {
  if (!selector) return;
  const valid = (u) => { try { const x = new URL(u, location.href); return /^https?:$/.test(x.protocol) ? x.href : ''; } catch (_) { return ''; } };
  tbOnDom(() => {
    for (const a of document.querySelectorAll(selector)) {
      if (a.dataset.tbSanitized) continue;
      let url = '';
      if (source === 'text') url = a.textContent.trim();
      else if (source.startsWith('?')) { try { url = new URL(a.href).searchParams.get(source.slice(1)) || ''; } catch (_) {} }
      else if (source.startsWith('[') && source.endsWith(']')) url = a.getAttribute(source.slice(1, -1)) || '';
      url = valid(url);
      if (url) { a.href = url; a.dataset.tbSanitized = '1'; }
    }
  }, true);
}

//@ nowebrtc +tb-util.fn
function tbNoWebRtc() {
  const noop = function() {};
  const Fake = function() {
    this.close = noop; this.createDataChannel = () => ({ close: noop, send: noop });
    this.createOffer = () => Promise.resolve({}); this.createAnswer = () => Promise.resolve({});
    this.setLocalDescription = () => Promise.resolve(); this.setRemoteDescription = () => Promise.resolve();
    this.addIceCandidate = () => Promise.resolve(); this.addEventListener = noop; this.removeEventListener = noop;
    this.getStats = () => Promise.resolve(new Map());
  };
  for (const k of ['RTCPeerConnection', 'webkitRTCPeerConnection']) {
    if (window[k]) { try { window[k] = Fake; } catch (_) {} }
  }
}
