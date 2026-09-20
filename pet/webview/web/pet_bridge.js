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
    }
  };

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
