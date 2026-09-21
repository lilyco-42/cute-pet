/*
 * pet_bridge.js —— Web 内容层与原生壳之间的双向桥(JS 侧)。
 *
 * 方向:
 *   JS -> Native : window.PetBridge.postMessage(JSON.stringify({id?, method, params}))
 *                  PetBridge 由 OverlayService 用 addJavascriptInterface 注入。
 *   Native -> JS : webView.evaluateJavascript("window.PetNative.onMessage({...})")
 *
 * 协议(method):
 *   move {dx,dy}                 移动悬浮窗
 *   setSize {w,h}                改窗口尺寸(dp)
 *   setPassthrough {on}          穿透模式(窗口不吃触摸 -> 下层应用可点)
 *   clipboard.get / clipboard.set
 *   keyboard.show {on}
 *   asset.fetch {path}  -> {encoding:"base64", data:"..."}   (M3 与 Rust 侧对接)
 *   capture.request     -> 屏幕帧(M3, 未实现)
 *   lifecycle {state:"ready"}     内核就绪
 *   log {level, msg}             回传 logcat
 *
 * 浏览器(Pages demo)里没有原生桥时, 所有调用降级为 no-op + 一次 console.warn,
 * 所以这份文件也可以直接放在 pages/ 下而不影响网页版。
 */
(function () {
  "use strict";

  var hasNative =
    typeof window.PetBridge !== "undefined" &&
    window.PetBridge &&
    typeof window.PetBridge.postMessage === "function";

  var seq = 0;
  var pending = {};
  var listeners = {};

  function post(obj) {
    if (!hasNative) {
      console.warn("[pet-bridge] 无原生桥(浏览器环境), 忽略:", obj);
      return;
    }
    try {
      window.PetBridge.postMessage(JSON.stringify(obj));
    } catch (e) {
      console.warn("[pet-bridge] postMessage 失败", e);
    }
  }

  function request(method, params, timeoutMs) {
    return new Promise(function (resolve, reject) {
      if (!hasNative) {
        reject(new Error("no native bridge: " + method));
        return;
      }
      var id = ++seq;
      pending[id] = { resolve: resolve, reject: reject };
      post({ id: id, method: method, params: params || {} });
      setTimeout(function () {
        if (pending[id]) {
          delete pending[id];
          reject(new Error("bridge timeout: " + method));
        }
      }, timeoutMs || 15000);
    });
  }

  window.PetNative = {
    /** 由 Native 调用(evaluateJavascript), msg 为 JSON 对象或字符串 */
    onMessage: function (msg) {
      try {
        msg = typeof msg === "string" ? JSON.parse(msg) : msg;
      } catch (e) {
        return;
      }
      if (!msg) return;

      if (msg.id && pending[msg.id]) {
        var p = pending[msg.id];
        delete pending[msg.id];
        if (msg.error) p.reject(new Error(msg.error));
        else p.resolve(msg.result);
        return;
      }

      var fns = listeners[msg.type];
      if (fns) {
        for (var i = 0; i < fns.length; i++) {
          try {
            fns[i](msg);
          } catch (e) {
            console.warn("[pet-bridge] listener error", e);
          }
        }
      }
    },
    /** 订阅 Native 主动推的事件: window.state / lifecycle / capture.frame */
    on: function (type, fn) {
      (listeners[type] = listeners[type] || []).push(fn);
    }
  };

  // ---------------- 网络导入(wasm 运行时资产注入) ----------------
  // 丛雨等第三方素材不进 wasm, 由本逻辑从 URL 拉取并经 cute_pet_register_asset 注入
  // wasm 线性内存; 之后引擎 load_asset 优先返回。导入结果存 IndexedDB, 页面重载/下次启动
  // 由 cutePetInjectStoredAssets 自动回注, 形成"粘性行为"(无需每次联网)。
  function rtAlloc(len) {
    return wasm_exports.cute_pet_alloc(len);
  }
  function rtRegister(path, bytes) {
    var enc = new TextEncoder();
    var p = enc.encode(path);
    var pPtr = rtAlloc(p.length);
    new Uint8Array(wasm_memory.buffer, pPtr, p.length).set(p);
    var dPtr = rtAlloc(bytes.length);
    new Uint8Array(wasm_memory.buffer, dPtr, bytes.length).set(bytes);
    wasm_exports.cute_pet_register_asset(pPtr, p.length, dPtr, bytes.length);
  }
  function idbOpen() {
    return new Promise(function (res, rej) {
      var r = indexedDB.open("cute-pet-assets", 1);
      r.onupgradeneeded = function () {
        r.result.createObjectStore("files");
        r.result.createObjectStore("meta");
      };
      r.onsuccess = function () { res(r.result); };
      r.onerror = function () { rej(r.error); };
    });
  }
  function idbPut(db, store, key, val) {
    return new Promise(function (res, rej) {
      var tx = db.transaction(store, "readwrite");
      tx.objectStore(store).put(val, key);
      tx.oncomplete = function () { res(); };
      tx.onerror = function () { rej(tx.error); };
    });
  }
  function idbGetAll(db) {
    return new Promise(function (res, rej) {
      var out = [];
      var tx = db.transaction("files", "readonly");
      var cur = tx.objectStore("files").openCursor();
      cur.onsuccess = function (e) {
        var c = e.target.result;
        if (c) { out.push([c.key, c.value]); c.continue(); }
        else { res(out); }
      };
      cur.onerror = function () { rej(cur.error); };
    });
  }
  // 启动/重载时回注已存素材(粘性行为); 在 index.html 调 main() 之前调用
  window.cutePetInjectStoredAssets = function () {
    return idbOpen().then(function (db) {
      return idbGetAll(db).then(function (all) {
        all.forEach(function (kv) { rtRegister(kv[0], kv[1]); });
      });
    }).catch(function () {});
  };
  // 从 pet-asset-bundle.json 地址导入一个素材包
  window.cutePetImportBundle = function (url) {
    return fetch(url).then(function (r) { return r.json(); }).then(function (manifest) {
      var base = url.slice(0, url.lastIndexOf("/") + 1);
      var files = manifest.files || [];
      return idbOpen().then(function (db) {
        return files.reduce(function (chain, rel) {
          return chain.then(function () {
            return fetch(base + rel).then(function (r) { return r.arrayBuffer(); }).then(function (ab) {
              var bytes = new Uint8Array(ab);
              return idbPut(db, "files", rel, bytes).then(function () { rtRegister(rel, bytes); });
            });
          });
        }, Promise.resolve()).then(function () {
          return idbPut(db, "meta", "bundleUrl", url);
        }).then(function () {
          location.reload();
        });
      });
    }).catch(function (e) {
      if (window.PetShell && window.PetShell.log) window.PetShell.log("导入失败: " + e, "error");
      console.error("cutePetImportBundle failed", e);
    });
  };

  window.PetShell = {
    isNative: function () {
      return hasNative;
    },
    ready: function () {
      post({ method: "lifecycle", params: { state: "ready" } });
    },
    move: function (dx, dy) {
      post({ method: "move", params: { dx: dx, dy: dy } });
    },
    setSize: function (w, h) {
      post({ method: "setSize", params: { w: w, h: h } });
    },
    setPassthrough: function (on) {
      post({ method: "setPassthrough", params: { on: !!on } });
    },
    keyboardShow: function (on) {
      post({ method: "keyboard.show", params: { on: !!on } });
    },
    clipboardGet: function () {
      return request("clipboard.get");
    },
    clipboardSet: function (text) {
      post({ method: "clipboard.set", params: { text: text } });
    },
    /** 向壳索取素材(角色素材不进 wasm, 由用户提供)。返回 {encoding,data} */
    fetchAsset: function (path) {
      return request("asset.fetch", { path: path }, 30000);
    },
    requestCapture: function () {
      return request("capture.request", {}, 30000);
    },
    log: function (msg, level) {
      post({ method: "log", params: { level: level || "info", msg: String(msg) } });
    },
    /** 网络导入: 从 pet-asset-bundle.json 地址拉取素材并注入 wasm(粘性行为) */
    importBundle: function (url) {
      return window.cutePetImportBundle(url);
    }
  };

  // ---------------- 默认桌宠当 agent 的脸 (MVP) ----------------
  // 渲染层: 原生大脑(后续接 lilyco-approve 的 router_v13 + AgentOps)通过
  //   evalJs("window.PetShell.agentSay(...)") / agentPropose(...)
  // 驱动; 浏览器演示用 JS mock agent(见 index.html)直接调这些函数, 不依赖原生桥。
  // 审批卡"批准/拒绝"按钮 -> agentApprove() -> 有原生桥则回传 agent.approve, 否则本地模拟。

  var bubbleEl = document.getElementById("agent-bubble");
  var cardEl = document.getElementById("agent-card");
  var cardTitleEl = cardEl ? cardEl.querySelector(".ac-title") : null;
  var cardActEl = cardEl ? cardEl.querySelector(".ac-act") : null;
  var currentActions = [];

  /** 动作类型 -> 中文描述(对齐 lilyco-approve 的 AgentOps.describe) */
  function describeAction(a) {
    if (!a) {
      return "(空)";
    }
    switch (a.type) {
      case "tap_text": return "点击文字：" + (a.text || "");
      case "tap": return "点击坐标 (" + (a.x | 0) + ", " + (a.y | 0) + ")";
      case "input": return "输入文字：" + (a.text || "");
      case "fill_text": return "填写 " + (a.label || "") + " = " + (a.text || "");
      case "open_app": return "打开应用：" + (a.label || a.pkg || "");
      case "back": return "返回";
      case "scroll_down": return "向下滚动";
      default: return String(a.type || "?");
    }
  }

  function renderBubble(text) {
    if (!bubbleEl) {
      return;
    }
    bubbleEl.textContent = text || "";
    bubbleEl.style.display = "block";
  }

  function renderCard(actions) {
    if (!cardEl) {
      return;
    }
    currentActions = actions || [];
    if (cardTitleEl) {
      cardTitleEl.textContent = "桌宠想执行以下操作：";
    }
    if (cardActEl) {
      cardActEl.innerHTML = "";
      currentActions.forEach(function (a) {
        var line = document.createElement("div");
        line.textContent = "· " + describeAction(a);
        cardActEl.appendChild(line);
      });
    }
    cardEl.style.display = "block";
    // 卡片出现时放大窗口(原生侧才生效, 浏览器里 setSize 是 no-op)
    if (hasNative) {
      window.PetShell.setSize(220, 360);
    }
  }

  function hideCard() {
    if (cardEl) {
      cardEl.style.display = "none";
    }
    if (hasNative) {
      window.PetShell.setSize(160, 280);
    }
  }

  function onAgentDecision(approved) {
    var verb = approved ? "批准" : "拒绝";
    hideCard();
    if (hasNative) {
      post({
        method: "agent.approve",
        params: { approved: !!approved, actions: currentActions }
      });
    } else {
      console.log("[mock agent] 用户" + verb + "了动作:", currentActions);
      renderBubble(approved ? "好嘞～我这就去办！" : "明白，那就不动啦。");
    }
  }

  // 审批卡按钮接线(只挂一次)
  if (cardEl) {
    var okBtn = cardEl.querySelector(".ac-ok");
    var noBtn = cardEl.querySelector(".ac-no");
    if (okBtn) {
      okBtn.addEventListener("click", function () { onAgentDecision(true); });
    }
    if (noBtn) {
      noBtn.addEventListener("click", function () { onAgentDecision(false); });
    }
  }

  // 暴露给原生桥(通过 evalJs 调用)与本地 mock agent
  window.PetShell.agentSay = renderBubble;
  window.PetShell.agentPropose = renderCard;
  window.PetShell.agentApprove = onAgentDecision;

  // JS 侧异常回传 logcat, 免得 adb 看不到 WebView 里的报错
  var origError = console.error;
  console.error = function () {
    try {
      post({
        method: "log",
        params: {
          level: "error",
          msg: Array.prototype.map.call(arguments, function (a) {
            return String(a);
          }).join(" ")
        }
      });
    } catch (e) {}
    origError.apply(console, arguments);
  };
})();
