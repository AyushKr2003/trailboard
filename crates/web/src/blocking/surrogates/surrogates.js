// Stand-ins served in place of blocked scripts (`$redirect=` filter rules),
// so a page that checks its ad or analytics script loaded finds something
// harmless there instead of an error. Our own, written from what the real
// scripts expose to pages. Each section starts with a `//@` line naming the
// resource and its aliases (the names filter lists use); `web::resources`
// reads them. Every section is a whole script that also works injected as a
// scriptlet (`+js(nobab)`).

//@ noop.js noopjs
(function() {})();

//@ googlesyndication_adsbygoogle.js googlesyndication.com/adsbygoogle.js googlesyndication-adsbygoogle
(function() {
  'use strict';
  const fill = () => {
    for (const ins of document.querySelectorAll('ins.adsbygoogle:not([data-adsbygoogle-status])')) {
      ins.setAttribute('data-adsbygoogle-status', 'done');
      ins.setAttribute('data-ad-status', 'unfilled');
      const frame = document.createElement('iframe');
      frame.style.cssText = 'border:0;width:100%;height:100%;display:block';
      frame.setAttribute('aria-hidden', 'true');
      frame.id = 'aswift_' + Math.floor(Math.random() * 1e6);
      ins.appendChild(frame);
    }
  };
  const queue = window.adsbygoogle;
  const stub = {
    loaded: true,
    push() { fill(); },
    length: 0,
  };
  window.adsbygoogle = stub;
  if (Array.isArray(queue)) for (let i = 0; i < queue.length; i++) stub.push(queue[i]);
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', fill, { once: true });
  else fill();
})();

//@ googletagservices_gpt.js googletagservices.com/gpt.js googletagservices-gpt
(function() {
  'use strict';
  const noop = function() {};
  const self = function() { return this; };
  const slot = (path, sizes, div) => {
    const targeting = new Map();
    const s = {
      addService: self, clearCategoryExclusions: self, clearTargeting() { targeting.clear(); return this; },
      defineSizeMapping: self, get: () => null, getAdUnitPath: () => path || '', getAttributeKeys: () => [],
      getCategoryExclusions: () => [], getDomId: () => div || '', getResponseInformation: () => null,
      getSlotElementId: () => div || '', getSlotId: () => s, getSizes: () => sizes || [],
      getTargeting: (k) => targeting.get(k) || [], getTargetingKeys: () => Array.from(targeting.keys()),
      getTargetingMap: () => Object.fromEntries(targeting), set: self, setCategoryExclusion: self,
      setClickUrl: self, setCollapseEmptyDiv: self, setConfig: self, setForceSafeFrame: self,
      setSafeFrameConfig: self, setTargeting(k, v) { targeting.set(k, [].concat(v)); return this; },
      updateTargetingFromMap: self, getEscapedQemQueryId: () => '', getHtml: () => '',
      getName: () => path || '', getOutOfPage: () => false, getContentUrl: () => '',
    };
    return s;
  };
  const listeners = [];
  const pubads = {
    addEventListener(type, fn) { listeners.push([type, fn]); return this; },
    removeEventListener: () => true, clear: () => true, clearCategoryExclusions: self,
    clearTagForChildDirectedTreatment: self, clearTargeting: self, collapseEmptyDivs: self,
    defineOutOfPagePassback: () => slot(), definePassback: () => slot(), disableInitialLoad: self,
    display: noop, enableAsyncRendering: self, enableLazyLoad: self, enableSingleRequest: self,
    enableSyncRendering: self, enableVideoAds: self, get: () => null, getAttributeKeys: () => [],
    getTargeting: () => [], getTargetingKeys: () => [], getSlots: () => [], isInitialLoadDisabled: () => false,
    refresh: noop, set: self, setCategoryExclusion: self, setCentering: self, setCookieOptions: self,
    setForceSafeFrame: self, setLocation: self, setPrivacySettings: self, setPublisherProvidedId: self,
    setRequestNonPersonalizedAds: self, setSafeFrameConfig: self, setTagForChildDirectedTreatment: self,
    setTargeting: self, setVideoContent: self, updateCorrelator: self,
  };
  const companionAds = { addEventListener: self, enableSyncLoading: noop, setRefreshUnfilledSlots: noop };
  const content = { addEventListener: self, setContent: noop };
  const sizeMapping = () => {
    const b = { addSize() { return b; }, build: () => [] };
    return b;
  };
  const old = window.googletag && window.googletag.cmd;
  const gt = window.googletag = window.googletag || {};
  const run = (f) => { try { typeof f === 'function' && f(); } catch (e) { setTimeout(() => { throw e; }); } };
  Object.assign(gt, {
    apiReady: true, pubadsReady: true, _loaded_: true,
    companionAds: () => companionAds, content: () => content, defineOutOfPageSlot: (p, d) => slot(p, [], d),
    defineSlot: (p, s, d) => slot(p, s, d), destroySlots: () => true, disablePublisherConsole: noop,
    display: noop, enableServices: noop, getVersion: () => '', pubads: () => pubads,
    setAdIframeTitle: noop, setConfig: noop, sizeMapping, getConfig: () => null,
    enums: { OutOfPageFormat: {}, TrafficSource: {} }, secureSignalProviders: { push: run, clearAllCache: noop },
    cmd: { push(...fs) { fs.forEach(run); return 0; } },
  });
  if (Array.isArray(old)) old.forEach(run);
})();

//@ google-analytics_analytics.js google-analytics.com/analytics.js googletagmanager_gtag.js googletagmanager.com/gtag/js
(function() {
  'use strict';
  const noop = function() {};
  // A hit's callback (pages often wait on it before following a link).
  const callback = (args) => {
    for (const a of args) {
      if (a && typeof a === 'object') {
        const cb = a.hitCallback || a.event_callback;
        if (typeof cb === 'function') setTimeout(cb, 1);
      }
    }
  };
  const tracker = { get: noop, set: noop, send(...a) { callback(a); } };
  const name = window.GoogleAnalyticsObject || 'ga';
  const queue = window[name] && window[name].q;
  const ga = function(...args) {
    if (typeof args[0] === 'function') { try { args[0](tracker); } catch (_) {} return; }
    callback(args);
  };
  ga.create = () => tracker;
  ga.getByName = () => tracker;
  ga.getAll = () => [tracker];
  ga.remove = noop;
  ga.loaded = true;
  window[name] = ga;
  if (Array.isArray(queue)) queue.forEach((a) => ga(...a));
  // gtag.js keeps its calls on `dataLayer`.
  const dl = window.dataLayer;
  if (dl && typeof dl.push === 'function') {
    const push = dl.push;
    dl.push = function(...items) {
      for (const it of items) callback(it && it.length !== undefined ? Array.from(it) : [it]);
      return push.apply(this, items);
    };
    for (const it of Array.from(dl)) callback(it && it.length !== undefined ? Array.from(it) : [it]);
  }
})();

//@ google-analytics_ga.js google-analytics.com/ga.js
(function() {
  'use strict';
  const noop = function() {};
  const tracker = new Proxy({}, { get: (_, k) => (k === '_getLinkerUrl' ? (u) => u : noop) });
  const gat = { _createTracker: () => tracker, _getTracker: () => tracker, _getTrackerByName: () => tracker,
    _anonymizeIp: noop, _forceSSL: noop, _getTrackers: () => [tracker] };
  window._gat = gat;
  const gaq = { push(...cmds) {
    for (const c of cmds) {
      if (typeof c === 'function') { try { c(); } catch (_) {} continue; }
      if (Array.isArray(c) && c[0] === '_link' && typeof c[1] === 'string') { try { location.href = c[1]; } catch (_) {} }
      if (Array.isArray(c) && typeof c[c.length - 1] === 'function' && c[0] === '_set' && c[1] === 'hitCallback') {
        try { c[2](); } catch (_) {}
      }
    }
  } };
  const old = window._gaq;
  window._gaq = gaq;
  if (Array.isArray(old)) gaq.push(...old);
})();

//@ googletagmanager_gtm.js googletagmanager.com/gtm.js
(function() {
  'use strict';
  const noop = function() {};
  window.ga = window.ga || noop;
  const dl = window.dataLayer;
  if (!dl || typeof dl.push !== 'function') return;
  const fire = (it) => {
    if (it && typeof it === 'object' && typeof it.eventCallback === 'function') setTimeout(it.eventCallback, 1);
  };
  dl.hide = undefined;
  const push = dl.push;
  dl.push = function(...items) { items.forEach(fire); return push.apply(this, items); };
  Array.from(dl).forEach(fire);
})();

//@ google-ima.js google-ima3 imasdk.googleapis.com/js/sdkloader/ima3.js
(function() {
  'use strict';
  // The IMA SDK, as far as a video player sees it: every ad request fails,
  // so the player goes straight to its video.
  if (window.google && window.google.ima && window.google.ima.VERSION) return;
  const noop = function() {};
  class Target {
    constructor() { this._l = new Map(); }
    addEventListener(types, fn, _capture, scope) {
      for (const t of [].concat(types)) {
        if (!this._l.has(t)) this._l.set(t, []);
        this._l.get(t).push(scope ? fn.bind(scope) : fn);
      }
    }
    removeEventListener(types, fn) {
      for (const t of [].concat(types)) this._l.set(t, (this._l.get(t) || []).filter((f) => f !== fn));
    }
    _fire(t, ev) { for (const f of (this._l.get(t) || []).slice()) { try { f(ev); } catch (e) { setTimeout(() => { throw e; }); } } }
  }
  class AdError {
    constructor(message, code, type) { this.message = message; this.errorCode = code; this.type = type; }
    getErrorCode() { return this.errorCode; }
    getVastErrorCode() { return this.errorCode; }
    getInnerError() { return null; }
    getMessage() { return this.message; }
    getType() { return this.type; }
    toString() { return 'AdError ' + this.errorCode + ': ' + this.message; }
  }
  AdError.ErrorCode = { VAST_EMPTY_RESPONSE: 1009, VAST_NO_ADS_AFTER_WRAPPER: 303, UNKNOWN_ERROR: 900 };
  AdError.Type = { AD_LOAD: 'adLoadError', AD_PLAY: 'adPlayError' };
  class AdErrorEvent {
    constructor(error, context) { this.error = error; this.context = context; this.type = AdErrorEvent.Type.AD_ERROR; }
    getError() { return this.error; }
    getUserRequestContext() { return this.context; }
  }
  AdErrorEvent.Type = { AD_ERROR: 'adError' };
  const adEventTypes = ['AD_BREAK_READY', 'AD_BUFFERING', 'AD_METADATA', 'AD_PROGRESS', 'ALL_ADS_COMPLETED',
    'CLICK', 'COMPLETE', 'CONTENT_PAUSE_REQUESTED', 'CONTENT_RESUME_REQUESTED', 'DURATION_CHANGE',
    'FIRST_QUARTILE', 'IMPRESSION', 'INTERACTION', 'LINEAR_CHANGED', 'LOADED', 'LOG', 'MIDPOINT', 'PAUSED',
    'RESUMED', 'SKIPPABLE_STATE_CHANGED', 'SKIPPED', 'STARTED', 'THIRD_QUARTILE', 'USER_CLOSE',
    'VIDEO_CLICKED', 'VIDEO_ICON_CLICKED', 'VOLUME_CHANGED', 'VOLUME_MUTED', 'AD_CAN_PLAY', 'IMPRESSION'];
  const AdEvent = function() {};
  AdEvent.Type = {};
  for (const t of adEventTypes) AdEvent.Type[t] = t.toLowerCase().replace(/_(\w)/g, (_, c) => c.toUpperCase());
  const AdsManagerLoadedEvent = function() {};
  AdsManagerLoadedEvent.Type = { ADS_MANAGER_LOADED: 'adsManagerLoaded' };
  class AdDisplayContainer { constructor() {} initialize() {} destroy() {} }
  class AdsRequest {
    constructor() {
      Object.assign(this, { adTagUrl: '', adsResponse: '', forceNonLinearFullSlot: false, linearAdSlotHeight: 0,
        linearAdSlotWidth: 0, nonLinearAdSlotHeight: 0, nonLinearAdSlotWidth: 0, vastLoadTimeout: 5000 });
    }
    setAdWillAutoPlay() {} setAdWillPlayMuted() {} setContinuousPlayback() {}
  }
  class AdsRenderingSettings { constructor() { this.enablePreloading = false; this.restoreCustomPlaybackStateOnAdBreakComplete = false; } }
  class ImaSdkSettings {
    constructor() { this._v = {}; }
    getCompanionBackfill() { return 'always'; } getDisableCustomPlaybackForIOS10Plus() { return false; }
    getFeatureFlags() { return {}; } getLocale() { return 'en'; } getNumRedirects() { return 4; }
    getPlayerType() { return ''; } getPlayerVersion() { return ''; } getPpid() { return ''; }
    isCookiesEnabled() { return false; } isVpaidAllowed() { return false; }
    setAutoPlayAdBreaks() {} setCompanionBackfill() {} setCookiesEnabled() {} setDisableCustomPlaybackForIOS10Plus() {}
    setFeatureFlags() {} setLocale() {} setNumRedirects() {} setPlayerType() {} setPlayerVersion() {}
    setPpid() {} setSessionId() {} setVpaidAllowed() {} setVpaidMode() {}
  }
  ImaSdkSettings.CompanionBackfillMode = { ALWAYS: 'always', ON_MASTER_AD: 'on_master_ad' };
  ImaSdkSettings.VpaidMode = { DISABLED: 0, ENABLED: 1, INSECURE: 2 };
  const settings = new ImaSdkSettings();
  class AdsLoader extends Target {
    constructor() { super(); }
    getSettings() { return settings; }
    contentComplete() {}
    destroy() {}
    getVersion() { return ima.VERSION; }
    requestAds(_request, context) {
      setTimeout(() => {
        const err = new AdError('No ads (blocked)', AdError.ErrorCode.VAST_EMPTY_RESPONSE, AdError.Type.AD_LOAD);
        this._fire(AdErrorEvent.Type.AD_ERROR, new AdErrorEvent(err, context));
      }, 1);
    }
  }
  const ima = {
    AdCuePoints: function() {}, AdDisplayContainer, AdError, AdErrorEvent, AdEvent, AdsLoader,
    AdsManagerLoadedEvent, AdsRenderingSettings, AdsRequest, ImaSdkSettings,
    CompanionAdSelectionSettings: function() {}, OmidAccessMode: { DOMAIN: 'domain', FULL: 'full', LIMITED: 'limited' },
    OmidVerificationVendor: {}, UiElements: { AD_ATTRIBUTION: 'adAttribution', COUNTDOWN: 'countdown' },
    UniversalAdIdInfo: function() {}, ViewMode: { FULLSCREEN: 'fullscreen', NORMAL: 'normal' },
    settings, VERSION: '3.517.2',
  };
  ima.CompanionAdSelectionSettings.CreativeType = { ALL: 'All', FLASH: 'Flash', IMAGE: 'Image' };
  ima.CompanionAdSelectionSettings.ResourceType = { ALL: 'All', HTML: 'Html', IFRAME: 'IFrame', STATIC: 'Static' };
  ima.CompanionAdSelectionSettings.SizeCriteria = { IGNORE: 'IgnoreSize', SELECT_EXACT_MATCH: 'SelectExactMatch', SELECT_NEAR_MATCH: 'SelectNearMatch' };
  ima.dai = { api: {} };
  window.google = window.google || {};
  window.google.ima = ima;
  void noop;
})();

//@ amazon_apstag.js amazon-adsystem.com/aax2/apstag.js amazon_ads.js amazon-adsystem.com/aax2/amzn_ads.js
(function() {
  'use strict';
  const noop = function() {};
  const queue = window.apstag && window.apstag._Q;
  window.apstag = {
    _Q: [], init: noop, deleteId: noop, renewId: noop, rpa: noop, upa: noop, dpa: noop, setDisplayBids: noop,
    targetingKeys: () => [],
    fetchBids(cfg, cb) { if (typeof cb === 'function') setTimeout(() => cb([]), 1); },
  };
  window.amznads = { getAds: noop, getTargeting: () => ({}), renderAd: noop, getTokens: noop, setTargetingForGPTAsync: noop };
  window.amzn_ads = noop;
  void queue;
})();

//@ doubleclick_instream_ad_status.js doubleclick.net/instream/ad_status.js
window.google_ad_status = 1;

//@ prebid-ads.js
(function() {
  'use strict';
  window.canRunAds = true;
  window.isAdBlockActive = false;
})();

//@ scorecardresearch_beacon.js scorecardresearch.com/beacon.js
(function() {
  'use strict';
  window.COMSCORE = { purge() { window._comscore = []; }, beacon() {} };
})();

//@ outbrain-widget.js widgets.outbrain.com/outbrain.js
(function() {
  'use strict';
  const noop = function() {};
  window.OBR = window.OBR || {};
  window.OBR.extern = new Proxy({}, { get: () => noop });
  window.outbrain = { callClick: noop, returnedHtmlData: noop, returnedIrdData: noop, returnedJsonData: noop };
})();

//@ fingerprint2.js
(function() {
  'use strict';
  const hash = '0123456789abcdef0123456789abcdef'.split('').sort(() => Math.random() - 0.5).join('');
  const Fp = function() {};
  Fp.get = (opts, cb) => { const done = typeof opts === 'function' ? opts : cb; if (done) setTimeout(() => done([]), 1); };
  Fp.getPromise = () => Promise.resolve([]);
  Fp.getV18 = (opts, cb) => { const done = typeof opts === 'function' ? opts : cb; if (done) setTimeout(() => done(hash, []), 1); };
  Fp.x64hash128 = () => hash;
  Fp.prototype.get = function(cb) { if (typeof cb === 'function') setTimeout(() => cb(hash, []), 1); };
  window.Fingerprint2 = Fp;
})();

//@ fingerprint3.js
(function() {
  'use strict';
  const visitorId = Math.random().toString(16).slice(2).padEnd(16, '0');
  const agent = { get: () => Promise.resolve({ visitorId, confidence: { score: 0.5 }, components: {}, version: '3.0.0' }) };
  window.FingerprintJS = { load: () => Promise.resolve(agent), hashComponents: () => visitorId, componentsToDebugString: () => '' };
})();

//@ nitropay_ads.js
(function() {
  'use strict';
  const noop = function() {};
  window.nitroAds = { createAd: () => Promise.resolve(null), addUserToken: noop, queue: [], loaded: true };
  document.dispatchEvent(new CustomEvent('nitroAds.loaded'));
})();

//@ adthrive_abd.js
window.adthriveABD = true;

//@ sensors-analytics.js
(function() {
  'use strict';
  const noop = function() {};
  window.sensorsDataAnalytic201505 = window.sensorsDataAnalytic201505 || 'sensors';
  window.sensors = new Proxy({ quick: noop, init: noop, track: noop, register: noop, login: noop, use: () => ({}) },
    { get: (t, k) => (k in t ? t[k] : noop) });
})();

//@ popads.js popads.net.js popads-dummy.js
(function() {
  'use strict';
  const stop = () => { throw new ReferenceError('popads'); };
  for (const k of ['PopAds', 'popns']) {
    try { Object.defineProperty(window, k, { get: stop, set: stop, configurable: false }); } catch (_) {}
  }
})();

//@ nobab.js bab-defuser.js
(function() {
  'use strict';
  // BlockAdBlock and its copies: report that nothing is blocked.
  const make = () => {
    const b = {
      _events: [],
      check() { this.emit(false); return true; },
      clearEvent() {}, emitEvent() { this.emit(false); return this; },
      emit(blocked) { for (const [want, fn] of this._events) { if (want === blocked) { try { fn(); } catch (_) {} } } },
      on(blocked, fn) { this._events.push([blocked, fn]); if (!blocked) setTimeout(fn, 1); return this; },
      onDetected() { return this; },
      onNotDetected(fn) { return this.on(false, fn); },
      setOption() { return this; },
    };
    return b;
  };
  for (const name of ['blockAdBlock', 'BlockAdBlock', 'sniffAdBlock', 'SniffAdBlock']) {
    try {
      const v = /^[A-Z]/.test(name) ? function() { return make(); } : make();
      Object.defineProperty(window, name, { value: v, writable: false, configurable: false });
    } catch (_) {}
  }
})();

//@ nobab2.js
(function() {
  'use strict';
  // The later BlockAdBlock: a random global holding a detector instance.
  const sig = /onDetected|onNotDetected|BlockAdBlock|bab_/;
  const realDefine = Object.defineProperty;
  const timers = window.setTimeout;
  window.setTimeout = new Proxy(timers, {
    apply(t, self, args) {
      const fn = args[0];
      if (typeof fn === 'function' && sig.test(String(fn))) args[0] = function() {};
      return Reflect.apply(t, self, args);
    },
  });
  void realDefine;
})();

//@ fuckadblock.js-3.2.0 nofab.js fuckadblock.js
(function() {
  'use strict';
  // FuckAdBlock / BlockAdBlock style detectors: report that nothing is blocked.
  const Fab = function() {};
  Fab.prototype = {
    check() { return this; },
    clearEvent() {},
    emitEvent() { return this; },
    on(blocked, fn) { if (!blocked && typeof fn === 'function') setTimeout(fn, 1); return this; },
    onDetected() { return this; },
    onNotDetected(fn) { if (typeof fn === 'function') setTimeout(fn, 1); return this; },
    setOption() { return this; },
    options: { set() { return this; }, get() { return undefined; } },
    debug: { set() { return this; }, get() { return false; } },
  };
  const fab = new Fab();
  for (const [k, v] of [['FuckAdBlock', Fab], ['BlockAdBlock', Fab], ['fuckAdBlock', fab], ['blockAdBlock', fab]]) {
    try { Object.defineProperty(window, k, { value: v, writable: false, configurable: false }); } catch (_) {}
  }
})();
