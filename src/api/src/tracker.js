/*! Lite CDP tracker — https://github.com/NatLee/lite-cdp (MIT) */
(function (window, document) {
  "use strict";
  var script = document.currentScript || document.querySelector("script[data-key][src*='t.js']");
  if (!script) return;
  var option = function (name) { return script.getAttribute("data-" + name); };
  var key = option("key");
  if (!key) return;
  var base = new URL(script.src, location.href).origin;
  var endpoint = option("endpoint") || base + "/collect";
  if (window.cdp && window.cdp.loaded) {
    if (window.cdp.key !== key || window.cdp.endpoint !== endpoint) window.cdp.diagnose("conflicting_installation");
    else window.cdp.duplicate();
    return;
  }
  var policy = null, active = false, consent = null, namespace = "", anonId = "", sessionId = "";
  var queue = [], identity = null, timer = null, lastUrl = null, reached = {}, removers = [];
  var inFlight = new Set(), readyCallbacks = [];
  var generation = 0, observedEpoch = "", testToken = option("test-token") || "";
  var diagnostics = { status: "loading", last_error: null, duplicates: 0, dropped: 0, attempts: 0, storage: "persistent" };
  var pending = window.cdpq || [];
  function emit() {
    if (window.dispatchEvent && typeof CustomEvent !== "undefined") window.dispatchEvent(new CustomEvent("cdp:diagnostic", { detail: api.status() }));
  }
  function diagnose(code) { diagnostics.last_error = code; if (window.console) window.console.warn("[Lite CDP] " + code); emit(); }
  function randomId() {
    var bytes = new Uint8Array(12); window.crypto.getRandomValues(bytes);
    return Array.prototype.map.call(bytes, function (b) { return ("0" + b.toString(16)).slice(-2); }).join("");
  }
  function read(storage, name) { try { return window[storage].getItem(name) || ""; } catch (_) { diagnostics.storage = "memory"; return ""; } }
  function write(storage, name, value) { try { window[storage].setItem(name, value); } catch (_) { diagnostics.storage = "memory"; } }
  function remove(storage, name) { try { window[storage].removeItem(name); } catch (_) {} }
  function stored(storage, suffix) {
    var name = namespace + suffix, value = read(storage, name);
    if (!value) { value = randomId(); write(storage, name, value); }
    return value;
  }
  function dnt() { return policy && policy.respect_dnt && (navigator.doNotTrack === "1" || window.doNotTrack === "1"); }
  function allowed() { return policy && policy.active !== false && !dnt() && consent !== false && (!policy.consent_required || consent === true); }
  function sanitize(value, scope) {
    if (!value || scope === "none") return "";
    try {
      var url = new URL(value, location.href);
      if (url.protocol !== "http:" && url.protocol !== "https:") return "";
      if (scope === "origin") return url.origin;
      url.hash = ""; url.username = ""; url.password = "";
      if (policy.query_mode !== "keep") {
        var params = new URLSearchParams();
        if (policy.query_mode === "allowlist") url.searchParams.forEach(function (v, k) { if ((policy.query_allowlist || []).indexOf(k) >= 0) params.append(k, v); });
        url.search = params.toString();
      }
      return url.href.slice(0, 2000);
    } catch (_) { return ""; }
  }
  function syncEpoch() {
    if (!active) return;
    var epoch = read("localStorage", namespace + "_epoch");
    if (epoch === observedEpoch) return;
    observedEpoch = epoch;
    var action; try { action = JSON.parse(epoch).action; } catch (_) {}
    if (action === "revoke") { consent = false; stop(); remove("sessionStorage", namespace + "_session_id"); return; }
    generation++; queue = []; identity = null;
    anonId = stored("localStorage", "_anon_id");
    remove("sessionStorage", namespace + "_session_id");
    sessionId = stored("sessionStorage", "_session_id");
  }
  function transport(payload, attempt, epoch, leaving) {
    if (!active || epoch !== generation) { inFlight.delete(payload); return; }
    var body;
    try { body = JSON.stringify(payload); }
    catch (_) { inFlight.delete(payload); diagnostics.dropped += payload.events.length; diagnose("invalid_payload"); return; }
    if (new Blob([body]).size > 60 * 1024) {
      inFlight.delete(payload);
      if (payload.events.length < 2) { diagnostics.dropped += payload.events.length; diagnose("payload_too_large"); return; }
      var middle = Math.ceil(payload.events.length / 2);
      var first = Object.assign({}, payload, {events:payload.events.slice(0,middle)});
      var second = Object.assign({}, payload, {events:payload.events.slice(middle)});
      transport(first,1,epoch,leaving); transport(second,1,epoch,leaving); return;
    }
    if (attempt === 1) { if (inFlight.size >= 8) { diagnostics.dropped += payload.events.length; diagnose("transport_capacity"); return; } inFlight.add(payload); }
    if (leaving && navigator.sendBeacon && navigator.sendBeacon(endpoint, new Blob([body], { type: "text/plain" }))) { inFlight.delete(payload); return; }
    diagnostics.attempts++;
    var controller = typeof AbortController !== "undefined" ? new AbortController() : null;
    var timeout;
    var request = window.fetch(endpoint, { signal: controller ? controller.signal : undefined, method: "POST", body: body, credentials: "omit", keepalive: true, headers: { "Content-Type": "text/plain" } });
    Promise.race([request, new Promise(function (_, reject) { timeout = setTimeout(function () { if (controller) controller.abort(); reject(new Error("network_timeout")); }, 10000); })])
      .then(function (res) {
        clearTimeout(timeout);
        if (!res.ok) { var err = new Error("collector_" + res.status); err.retry = res.status === 429 || res.status >= 500; throw err; }
        inFlight.delete(payload); if (active && epoch === generation) { diagnostics.status = "collecting"; emit(); }
      }).catch(function (err) {
        clearTimeout(timeout);
        if (!active || epoch !== generation) { inFlight.delete(payload); return; }
        if (attempt < 3 && err.retry !== false) setTimeout(function () { transport(payload, attempt + 1, epoch, false); }, 500 * Math.pow(2, attempt));
        else { inFlight.delete(payload); diagnostics.dropped += payload.events.length; diagnose(err.message || "network_error"); }
      });
  }
  function flush(leaving) {
    clearTimeout(timer); timer = null; syncEpoch();
    if (!active || (!queue.length && !identity)) return;
    var payload = { key: key, anon_id: anonId, session_id: sessionId, consent: consent === true, events: queue.splice(0, 50) };
    if (identity) { payload.identify = identity; identity = null; }
    if (testToken) payload.test_token = testToken;
    transport(payload, 1, generation, leaving);
    if (queue.length) flush(leaving);
  }
  function schedule() { if (queue.length >= 20) flush(false); else if (!timer) timer = setTimeout(function () { flush(false); }, 800); }
  function push(type, name, props) {
    syncEpoch(); if (!active) return;
    if (queue.length >= 200) { diagnostics.dropped++; diagnose("queue_full"); return; }
    var event = { event_id: randomId(), happened_at: new Date().toISOString(), type: type, url: sanitize(location.href), title: document.title.slice(0, 300) };
    if (name) event.name = String(name).slice(0, 120);
    if (props && typeof props === "object") event.props = props;
    queue.push(event); schedule(); return event;
  }
  function routeUrl() { return policy.spa_mode === "hash" ? location.href : location.href.split("#")[0]; }
  function page() {
    var event = push("pageview");
    if (event) event.referrer = sanitize(lastUrl === null ? document.referrer : lastUrl, policy.referrer_scope);
    if (active) { lastUrl = routeUrl(); reached = {}; }
  }
  function listen(target, name, handler, opts) {
    target.addEventListener(name, handler, opts);
    removers.push(function () { target.removeEventListener(name, handler, opts); });
  }
  function stop() {
    active = false; generation++; inFlight.clear(); clearTimeout(timer); timer = null; queue = []; identity = null;
    removers.splice(0).forEach(function (removeListener) { removeListener(); });
    diagnostics.status = dnt() ? "dnt" : "consent_required"; emit();
  }
  function eraseIdentity(broadcast) {
    if (!namespace) return;
    remove("localStorage", namespace + "_anon_id"); remove("sessionStorage", namespace + "_session_id");
    if (read("localStorage", "cdp_legacy_claim") === namespace) remove("localStorage", "cdp_anon_id");
    if (broadcast) write("localStorage", namespace + "_epoch", JSON.stringify({action:"revoke",at:Date.now()}));
  }
  function start() {
    if (active || !allowed()) { if (!active) { if (dnt()) eraseIdentity(false); diagnostics.status = dnt() ? "dnt" : policy && policy.active === false ? "paused" : "consent_required"; emit(); } return; }
    // Only one site may claim the old origin-wide identifier. No storage is touched before permission.
    var claimant = read("localStorage", "cdp_legacy_claim");
    if (!claimant && read("localStorage", "cdp_anon_id")) {
      write("localStorage", namespace + "_anon_id", read("localStorage", "cdp_anon_id"));
      write("localStorage", "cdp_legacy_claim", namespace);
    }
    anonId = stored("localStorage", "_anon_id"); sessionId = stored("sessionStorage", "_session_id");
    observedEpoch = read("localStorage", namespace + "_epoch"); active = true; lastUrl = null; diagnostics.status = "ready";
    var changed = function () { setTimeout(function () { if (active && routeUrl() !== lastUrl) page(); }, 0); };
    if (policy.auto_page && policy.spa_mode !== "manual") {
      ["pushState", "replaceState"].forEach(function (method) {
        var original = history[method]; if (typeof original !== "function") return;
        var wrapped = function () { var result = original.apply(this, arguments); changed(); return result; };
        history[method] = wrapped; removers.push(function () { if (history[method] === wrapped) history[method] = original; });
      });
      listen(window, "popstate", changed);
      if (policy.spa_mode === "hash") listen(window, "hashchange", changed);
    }
    if (policy.auto_click) listen(document, "click", function (e) {
      var target = e.target && e.target.closest ? e.target.closest("a,button,[data-cdp-click]") : null;
      if (!target || target.closest("[data-cdp-ignore]")) return;
      if ((policy.ignore_selectors || []).some(function (selector) { try { return target.closest(selector); } catch (_) { diagnose("invalid_ignore_selector"); return false; } })) return;
      var explicit = target.getAttribute("data-cdp-click");
      var name = explicit || (policy.click_text ? target.innerText || target.getAttribute("aria-label") : target.tagName.toLowerCase());
      var props = { element: target.tagName.toLowerCase() };
      if (target.href) props.href = sanitize(target.href);
      push("click", String(name || "click").replace(/\s+/g, " ").trim().slice(0, 80), props);
    }, true);
    if (policy.auto_scroll) {
      var measuring = false;
      listen(window, "scroll", function () {
        if (measuring) return; measuring = true;
        setTimeout(function () {
          measuring = false; if (!active) return;
          var root = document.documentElement, height = Math.max(root.scrollHeight, document.body ? document.body.scrollHeight : 0), view = window.innerHeight || root.clientHeight;
          if (height <= view + 50) return;
          var depth = ((window.pageYOffset || root.scrollTop || 0) + view) / height;
          [25,50,75,100].forEach(function (mark) { if (!reached[mark] && depth * 100 >= mark - 0.5) { reached[mark] = true; push("scroll", mark + "%", { depth: mark }); } });
        }, 250);
      }, { passive: true });
    }
    listen(window, "pagehide", function () { flush(true); });
    listen(document, "visibilitychange", function () { if (document.visibilityState === "hidden") flush(true); });
    listen(window, "storage", syncEpoch);
    if (policy.auto_page) page(); emit();
  }
  var api = {
    loaded: true, key: key, endpoint: endpoint,
    status: function () { return Object.assign({}, diagnostics, { policy: policy, consent: consent, queue_size: queue.length, namespace: namespace }); },
    diagnose: diagnose,
    ready: function (callback) { if (typeof callback !== "function") return; if (policy) callback(api); else readyCallbacks.push(callback); },
    duplicate: function () { diagnostics.duplicates++; emit(); },
    consent: function (granted) {
      consent = granted === true;
      if (consent) start();
      else {
        stop(); eraseIdentity(true);
      }
    },
    identify: function (who) {
      syncEpoch(); if (!active || !who || typeof who !== "object") return;
      var next = { traits: {} };
      Object.keys(who).forEach(function (field) {
        var value = who[field]; if (value === undefined || value === null) return;
        if (["user_id","email","name","account","proof"].indexOf(field) >= 0) next[field] = field === "proof" ? value : String(value);
        else if (policy.allowed_traits === null || (policy.allowed_traits || []).indexOf(field) >= 0) next.traits[field] = value;
      });
      // Preserve attribution boundaries when a different login arrives in the same batch.
      if (identity) flush(false); identity = next; schedule();
    },
    track: function (name, props) { if (name) push("custom", name, props); },
    page: page,
    reset: function () {
      if (!active) return; flush(false); generation++;
      remove("localStorage", namespace + "_anon_id"); remove("sessionStorage", namespace + "_session_id");
      anonId = stored("localStorage", "_anon_id"); sessionId = stored("sessionStorage", "_session_id");
      observedEpoch = JSON.stringify({action:"reset",at:Date.now(),nonce:randomId()});
      write("localStorage", namespace + "_epoch", observedEpoch);
    },
    test: function (token) { if (queue.length || identity) flush(false); testToken = String(token || ""); },
    flush: function () { flush(false); }
  };
  window.cdp = api;
  function run(call) { if (call && typeof api[call[0]] === "function") api[call[0]].apply(api, [].slice.call(call, 1)); }
  window.cdpq = { push: run };
  // Consent can be set before config arrives; tracking calls before permission are deliberately discarded.
  for (var i = 0; i < pending.length; i++) if (["consent", "ready"].indexOf(pending[i][0]) >= 0) run(pending[i]);
  window.fetch(base + "/tracker-config/" + encodeURIComponent(key), { credentials: "omit" })
    .then(function (res) { if (!res.ok) throw new Error("configuration_unavailable"); return res.json(); })
    .then(function (config) {
      policy = config.policy; policy.active = config.active; namespace = "cdp_" + config.namespace;
      ["auto-click","auto-scroll","auto-page"].forEach(function (attr) { if (option(attr) === "false") policy[attr.replace(/-/g,"_")] = false; });
      if (option("respect-dnt") === "true") policy.respect_dnt = true;
      if (["history","hash","manual"].indexOf(option("spa-mode")) >= 0) policy.spa_mode = option("spa-mode");
      if (consent === false) eraseIdentity(true);
      start();
      readyCallbacks.splice(0).forEach(function (callback) { callback(api); });
      if (active) for (var j = 0; j < pending.length; j++) if (["consent", "ready"].indexOf(pending[j][0]) < 0) run(pending[j]);
    }).catch(function (err) { diagnostics.status = "error"; diagnose(err.message); });
})(window, document);
