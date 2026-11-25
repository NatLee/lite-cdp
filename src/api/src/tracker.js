/*! Lite CDP tracker — https://github.com/NatLee/lite-cdp (MIT) */
(function (window, document) {
  "use strict";

  if (window.cdp && window.cdp.loaded) return;

  var script = document.currentScript || document.querySelector("script[data-key][src*='t.js']");
  var key = script && script.getAttribute("data-key");
  if (!key) return;

  function option(name) {
    return script.getAttribute("data-" + name);
  }

  var endpoint = option("endpoint") || new URL(script.src, location.href).origin + "/collect";
  var doNotTrack = navigator.doNotTrack === "1" || window.doNotTrack === "1";
  var disabled = option("respect-dnt") === "true" && doNotTrack;

  function randomId() {
    var bytes = new Uint8Array(12);
    (window.crypto || window.msCrypto).getRandomValues(bytes);
    var id = "";
    for (var i = 0; i < bytes.length; i++) id += ("0" + bytes[i].toString(16)).slice(-2);
    return id;
  }

  // 瀏覽器不給用 storage（無痕模式、被擋）時，退回只活在這一頁的編號
  function stored(storage, name, renew) {
    try {
      var value = renew ? null : window[storage].getItem(name);
      if (!value) {
        value = randomId();
        window[storage].setItem(name, value);
      }
      return value;
    } catch (e) {
      return randomId();
    }
  }

  var anonId = stored("localStorage", "cdp_anon_id");
  var sessionId = stored("sessionStorage", "cdp_session_id");

  var queue = [];
  var identity = null;
  var timer = null;

  function send(body, leaving) {
    if (leaving && navigator.sendBeacon) {
      // text/plain 不會觸發 CORS 預檢
      if (navigator.sendBeacon(endpoint, new Blob([body], { type: "text/plain" }))) return;
    }
    if (window.fetch) {
      fetch(endpoint, {
        method: "POST",
        body: body,
        keepalive: true,
        credentials: "omit",
        headers: { "Content-Type": "text/plain" },
      }).catch(function () {});
    } else if (navigator.sendBeacon) {
      navigator.sendBeacon(endpoint, new Blob([body], { type: "text/plain" }));
    }
  }

  function flush(leaving) {
    clearTimeout(timer);
    timer = null;
    if (!queue.length && !identity) return;
    var payload = { key: key, anon_id: anonId, session_id: sessionId, events: queue.splice(0, 50) };
    if (identity) {
      payload.identify = identity;
      identity = null;
    }
    send(JSON.stringify(payload), leaving);
    if (queue.length) flush(leaving);
  }

  function schedule() {
    if (queue.length >= 20) return flush(false);
    if (!timer) {
      timer = setTimeout(function () {
        flush(false);
      }, 800);
    }
  }

  function push(type, name, props) {
    if (disabled) return;
    var event = { type: type, url: location.href, title: document.title };
    if (name) event.name = String(name).slice(0, 120);
    if (props && typeof props === "object") event.props = props;
    queue.push(event);
    schedule();
    return event;
  }

  // ---- 瀏覽頁面（單頁應用換網址也算）----

  var lastUrl = null;
  var reached = {};

  function pageUrl() {
    return location.href.split("#")[0];
  }

  function page() {
    var event = push("pageview");
    if (event) event.referrer = lastUrl === null ? document.referrer : lastUrl;
    lastUrl = pageUrl();
    reached = {};
  }

  function pageChanged() {
    // 等框架把標題換好
    setTimeout(function () {
      if (pageUrl() !== lastUrl) page();
    }, 0);
  }

  ["pushState", "replaceState"].forEach(function (method) {
    var original = history[method];
    if (typeof original !== "function") return;
    history[method] = function () {
      var result = original.apply(this, arguments);
      pageChanged();
      return result;
    };
  });
  window.addEventListener("popstate", pageChanged);

  // ---- 捲動到 25 / 50 / 75 / 100% ----

  var measuring = false;

  function measure() {
    measuring = false;
    var root = document.documentElement;
    var height = Math.max(root.scrollHeight, document.body ? document.body.scrollHeight : 0);
    var view = window.innerHeight || root.clientHeight;
    // 不用捲就看得完的頁面不算
    if (height <= view + 50) return;
    var depth = ((window.pageYOffset || root.scrollTop) + view) / height;
    [25, 50, 75, 100].forEach(function (mark) {
      if (!reached[mark] && depth * 100 >= mark - 0.5) {
        reached[mark] = true;
        push("scroll", mark + "%", { depth: mark });
      }
    });
  }

  if (option("auto-scroll") !== "false") {
    window.addEventListener(
      "scroll",
      function () {
        if (measuring) return;
        measuring = true;
        setTimeout(measure, 250);
      },
      { passive: true }
    );
  }

  // ---- 點連結和按鈕 ----

  function text(value) {
    return String(value || "").replace(/\s+/g, " ").trim().slice(0, 80);
  }

  if (option("auto-click") !== "false") {
    document.addEventListener(
      "click",
      function (e) {
        var target = e.target && e.target.closest ? e.target.closest("a, button, [data-cdp-click]") : null;
        if (!target || target.closest("[data-cdp-ignore]")) return;
        var name =
          text(target.getAttribute("data-cdp-click")) ||
          text(target.innerText) ||
          text(target.getAttribute("aria-label")) ||
          text(target.getAttribute("title"));
        if (!name) return;
        var props = { element: target.tagName.toLowerCase() };
        if (target.href && typeof target.href === "string") props.href = target.href.slice(0, 500);
        push("click", name, props);
      },
      true
    );
  }

  // ---- 離開頁面前把還沒送的送出去 ----

  window.addEventListener("pagehide", function () {
    flush(true);
  });
  document.addEventListener("visibilitychange", function () {
    if (document.visibilityState === "hidden") flush(true);
  });

  // ---- 給網站呼叫的 API ----

  var api = {
    loaded: true,
    /** 告訴 CDP 這個訪客是誰：cdp.identify({ email, name, account, 其他特徵... }) */
    identify: function (who) {
      if (disabled || !who || typeof who !== "object") return;
      var next = identity || { traits: {} };
      Object.keys(who).forEach(function (field) {
        var value = who[field];
        if (value === undefined || value === null) return;
        if (field === "email" || field === "name" || field === "account") next[field] = String(value);
        else next.traits[field] = value;
      });
      identity = next;
      schedule();
    },
    /** 自訂事件：cdp.track("add_to_cart", { sku: "A-001" }) */
    track: function (name, props) {
      if (name) push("custom", name, props);
    },
    /** 手動記一次瀏覽（自己處理換頁的網站用） */
    page: page,
    /** 登出時呼叫：之後的事件算在新的匿名訪客身上 */
    reset: function () {
      flush(false);
      anonId = stored("localStorage", "cdp_anon_id", true);
      sessionId = stored("sessionStorage", "cdp_session_id", true);
    },
  };

  // 追蹤程式碼還沒載完之前，可以先 cdpq.push(["identify", {...}])
  var pending = window.cdpq || [];
  function run(call) {
    if (call && typeof api[call[0]] === "function") api[call[0]].apply(api, [].slice.call(call, 1));
  }
  window.cdp = api;
  window.cdpq = { push: run };
  for (var i = 0; i < pending.length; i++) run(pending[i]);

  page();
})(window, document);
