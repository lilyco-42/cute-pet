// 壳内中文 TTS 引擎（WebView 内合成，无需外部服务）—— kokoro-js-zh + espeak-ng(中文 G2P)
//
// 🚫 **已下线(2026-09-23): 本文件不再随壳分发, 保留在仓库仅作归档。**
//   build.sh 不再拷贝它, verify_apk.sh 有断言禁止它进包。壳内离线中文语音
//   由原生 sherpa-onnx 承担(见 docs/webview-shell-android.md §14)。
//   如需复活本条路线: 需同时补回 vendor/(espeak-ng.wasm + kokoro.web.js)、
//   voices/*.bin, 并解决下面的音素集问题。
//
// ⚠️⚠️ 质量警告：本引擎**默认关闭**，不建议用于生产 ⚠️⚠️
//   实测该社区包的音素集（espeak）与模型期望的 misaki[zh] 不一致 →
//   合成是「**后半句准、前半句糊**」，且前半句带明显**英文口音**（用户实听确认）。
//   官方 Python 管线（kokoro + misaki[zh]）同一句可做到逐字全对 —— 差距在 G2P。
//   详见 docs/webview-shell-android.md §13.1.2。
//   开启方式（仅调试/实验）：URL 带 `?tts_local=1`，或 localStorage['pet_tts_local']='1'。
//
// 依赖（已随壳分发，见 vendor/kokoro-zh/）:
//   - kokoro.web.js   自带 transformers.js，无外部 import
//   - espeak-ng.wasm  必须与 kokoro.web.js 同目录（代码用 new URL(..., import.meta.url) 定位）
//   - voices/*.bin    音色**必须放本地**（同页面相对路径 ./voices/），否则会拿到 404 的 HTML
//                     被当成浮点数据 → RangeError（见 §13.1.1）
(function () {
  var MODEL = 'onnx-community/Kokoro-82M-v1.0-ONNX';
  var VOICE = 'zf_xiaobei';
  var tts = null;
  var loading = null;
  var audioEl = null;
  var enabled = false;

  // 默认关闭；显式开启才生效
  try {
    var qs = location.search;
    if (/[?&]tts_local=1\b/.test(qs) || localStorage.getItem('pet_tts_local') === '1') {
      enabled = true;
    }
  } catch (e) {}

  // ⚠️ 只支持 wasm 后端：实测 WebGPU EP 对本模型会输出**静音+削顶尖刺**的坏音频
  // （volumedetect: max=0.0dB / mean=-32dB），wasm 后端正常（max≈-4dB）。
  // 因此这里不再探测 navigator.gpu。
  function pickDevice() {
    return 'wasm';
  }

  function load() {
    if (tts) {
      return Promise.resolve(tts);
    }
    if (loading) {
      return loading;
    }
    loading = import('./vendor/kokoro-zh/kokoro.web.js')
      .then(function (m) {
        var device = pickDevice();
        var dtype = device === 'webgpu' ? 'fp32' : 'q8';
        console.log('[tts-local] loading ' + MODEL + ' device=' + device + ' dtype=' + dtype);
        return m.KokoroTTS.from_pretrained(MODEL, { dtype: dtype, device: device });
      })
      .then(function (t) {
        tts = t;
        console.log('[tts-local] ready; voices=' + Object.keys(t.voices || {}).length);
        return t;
      })
      .catch(function (e) {
        loading = null; // 允许重试
        console.warn('[tts-local] load failed: ' + e);
        throw e;
      });
    return loading;
  }

  function play(audio) {
    var url = null;
    try {
      if (audio && typeof audio.toBlob === 'function') {
        url = URL.createObjectURL(audio.toBlob());
      } else if (audio && typeof audio.toWav === 'function') {
        url = URL.createObjectURL(new Blob([audio.toWav()], { type: 'audio/wav' }));
      }
    } catch (e) {
      url = null;
    }
    if (!url) {
      return false;
    }
    if (audioEl) {
      try { audioEl.pause(); } catch (e) {}
    }
    var a = new Audio(url);
    a.onended = function () { URL.revokeObjectURL(url); };
    audioEl = a;
    var p = a.play();
    if (p && p.catch) { p.catch(function () {}); } // 自动播放被拦 → 静默
    return true;
  }

  // 合成并播放；resolve(true)=已播放, resolve(false)=未播放(交给上层兜底)
  function speak(text) {
    if (!enabled) {
      return Promise.resolve(false); // 默认关闭 → 交给上层(HTTP/静默)
    }
    var t = (text == null ? '' : String(text)).trim();
    if (!t) {
      return Promise.resolve(false);
    }
    return load()
      .then(function (inst) { return inst.generate(t, { voice: VOICE }); })
      .then(function (audio) { return play(audio); })
      .catch(function () { return false; });
  }

  function stop() {
    if (audioEl) {
      try { audioEl.pause(); } catch (e) {}
      audioEl = null;
    }
  }

  window.cutePetTTSLocal = {
    model: MODEL,
    voice: VOICE,
    /** ⚠️ 默认 false —— 质量不达标（前糊后准 + 英文口音），仅调试用 */
    enabled: function () { return enabled; },
    setEnabled: function (on) {
      enabled = !!on;
      try { localStorage.setItem('pet_tts_local', enabled ? '1' : '0'); } catch (e) {}
      return enabled;
    },
    ready: function () { return !!tts; },
    load: load,
    speak: speak,
    stop: stop
  };
})();
