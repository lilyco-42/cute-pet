package rust.cute_pet;

import android.os.Handler;
import android.os.Looper;
import android.util.Base64;
import android.webkit.JavascriptInterface;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;

/**
 * JS <-> Native 桥(Android 侧)。
 *
 * 入站(JS -> Native): addJavascriptInterface("PetBridge"), JS 调
 *   PetBridge.postMessage(JSON.stringify({id?, method, params}))
 * 出站(Native -> JS): webView.evaluateJavascript("window.PetNative.onMessage({...})")
 *
 * 协议见 pet/webview/web/pet_bridge.js 顶部注释。所有结果都回到主线程执行。
 */
public final class PetBridge {

    /** 桥需要的系统能力, 由 OverlayService 实现 */
    public interface Host {
        void evalJs(String js);

        void moveBy(int dx, int dy);

        void setSizeDp(int w, int h);

        void setPassthrough(boolean on);

        String getClipboard();

        void setClipboard(String text);

        void showKeyboard(boolean show);

        void requestCapture();

        /** 素材目录候选(按优先级) */
        File[] assetDirs();

        // ---- agent 脸(MVP): 默认桌宠当界面, 真大脑(lilyco-approve)后续接入 ----

        /** 原生大脑驱动桌宠说一句台词 */
        void agentSay(String text);

        /** 原生大脑让桌宠弹出审批卡(actions 为动作对象数组, schema 见 docs) */
        void agentPropose(JSONArray actions);

        /** JS 审批卡"批准/拒绝"后回传: approved + 原样动作数组 */
        void onAgentApproved(boolean approved, JSONArray actions);

        /** 网络导入: 从 pet-asset-bundle.json 地址拉取素材并注入 wasm(url 为空则用预设) */
        void importBundle(String url);

        // ---- 壳内离线 TTS(sherpa-onnx, 见 TtsEngine) ----

        /** 说一句话(新请求顶掉旧的, 非阻塞; 引擎未就绪时排队) */
        void ttsSpeak(String text);

        /** 停止当前播放并清空待说队列 */
        void ttsStop();
    }

    private static final String TAG = "PetBridge";

    private final Host host;
    private final Handler main = new Handler(Looper.getMainLooper());

    PetBridge(Host host) {
        this.host = host;
    }

    // ---------------- JS -> Native ----------------

    @JavascriptInterface
    public void postMessage(String json) {
        try {
            JSONObject o = new JSONObject(json);
            final String method = o.optString("method", "");
            final JSONObject params = o.optJSONObject("params");
            final int id = o.optInt("id", 0);
            main.post(() -> handle(method, params == null ? new JSONObject() : params, id));
        } catch (Exception e) {
            android.util.Log.e(TAG, "bad message: " + json, e);
        }
    }

    /** 同步查询: 让 JS 判断自己是不是真的跑在壳里 */
    @JavascriptInterface
    public String shellInfo() {
        try {
            JSONObject o = new JSONObject();
            o.put("platform", "android");
            o.put("passthroughSupported", true);
            o.put("assetFetch", true);
            o.put("capture", true); // M3: PixelCopy 截 WebView 自身画面 -> base64 推 JS
            o.put("nativeTts", true); // 壳内离线 TTS 可用(引擎在后台 init, speak 会排队)
            return o.toString();
        } catch (Exception e) {
            return "{}";
        }
    }

    private void handle(String method, JSONObject p, int id) {
        try {
            switch (method) {
                case "move":
                    host.moveBy(p.optInt("dx"), p.optInt("dy"));
                    reply(id, null, null);
                    break;

                case "setSize":
                    host.setSizeDp(p.optInt("w"), p.optInt("h"));
                    reply(id, null, null);
                    break;

                case "setPassthrough":
                    host.setPassthrough(p.optBoolean("on", false));
                    reply(id, null, null);
                    break;

                case "clipboard.get": {
                    String t = host.getClipboard();
                    JSONObject r = new JSONObject();
                    r.put("text", t == null ? "" : t);
                    reply(id, r, null);
                    break;
                }

                case "clipboard.set":
                    host.setClipboard(p.optString("text", ""));
                    reply(id, null, null);
                    break;

                case "keyboard.show":
                    host.showKeyboard(p.optBoolean("on", false));
                    reply(id, null, null);
                    break;

                case "asset.fetch": {
                    String path = p.optString("path", "");
                    try {
                        byte[] data = readAssetFile(path);
                        JSONObject r = new JSONObject();
                        r.put("encoding", "base64");
                        r.put("size", data.length);
                        r.put("data", Base64.encodeToString(data, Base64.NO_WRAP));
                        reply(id, r, null);
                    } catch (Exception e) {
                        reply(id, null, "asset.fetch 失败(" + path + "): " + e.getMessage());
                    }
                    break;
                }

                case "capture.request":
                    host.requestCapture();
                    reply(id, null, "capture 尚未实现(M3)");
                    break;

                case "lifecycle":
                    android.util.Log.i(TAG, "lifecycle: " + p.optString("state"));
                    break;

                case "agent.approve": {
                    JSONArray acts = p.optJSONArray("actions");
                    host.onAgentApproved(p.optBoolean("approved"), acts);
                    reply(id, null, null);
                    break;
                }

                case "log":
                    android.util.Log.i(TAG, "[" + p.optString("level", "info") + "] " + p.optString("msg"));
                    break;

                case "tts.speak":
                    host.ttsSpeak(p.optString("text", ""));
                    reply(id, null, null);
                    break;

                case "tts.stop":
                    host.ttsStop();
                    reply(id, null, null);
                    break;

                default:
                    reply(id, null, "未知 method: " + method);
                    break;
            }
        } catch (Exception e) {
            reply(id, null, String.valueOf(e));
        }
    }

    // ---------------- Native -> JS ----------------

    private void reply(int id, JSONObject result, String error) {
        if (id == 0 && error == null) {
            return; // 单向调用, 不需要回包
        }
        try {
            JSONObject o = new JSONObject();
            o.put("id", id);
            if (error != null) {
                o.put("error", error);
            } else if (result != null) {
                o.put("result", result);
            }
            String js = "window.PetNative&&window.PetNative.onMessage(" + o.toString() + ")";
            host.evalJs(js);
        } catch (Exception e) {
            android.util.Log.e(TAG, "reply failed", e);
        }
    }

    /** 主动推事件给 JS(type: window.state / lifecycle / capture.frame) */
    void push(String type, JSONObject payload) {
        try {
            JSONObject o = new JSONObject();
            o.put("type", type);
            if (payload != null) {
                o.put("payload", payload);
            }
            String js = "window.PetNative&&window.PetNative.onMessage(" + o.toString() + ")";
            host.evalJs(js);
        } catch (Exception e) {
            android.util.Log.e(TAG, "push failed", e);
        }
    }

    // ---------------- 素材 ----------------

    /**
     * 从用户素材目录读文件。角色素材不随分发产物走(合规), 只可能来自用户自备目录
     * (见 THIRD-PARTY-NOTICES.md §2.2)。命中顺序与 Rust external_asset_dirs() 一致。
     */
    private byte[] readAssetFile(String path) throws Exception {
        if (path == null || path.isEmpty()) {
            throw new IllegalArgumentException("empty path");
        }
        String norm = new File(path).getPath();
        if (norm.contains("..")) {
            throw new SecurityException("非法路径: " + path);
        }
        for (File dir : host.assetDirs()) {
            if (dir == null) {
                continue;
            }
            File f = new File(dir, norm);
            if (f.isFile()) {
                return readAll(f);
            }
        }
        throw new java.io.FileNotFoundException(
                "素材未找到: " + path + " (已查找: " + joinDirs() + ")");
    }

    private String joinDirs() {
        StringBuilder sb = new StringBuilder();
        for (File d : host.assetDirs()) {
            if (sb.length() > 0) {
                sb.append(", ");
            }
            sb.append(d == null ? "(null)" : d.getAbsolutePath());
        }
        return sb.toString();
    }

    private static byte[] readAll(File f) throws Exception {
        ByteArrayOutputStream bos = new ByteArrayOutputStream();
        try (InputStream in = new FileInputStream(f)) {
            byte[] buf = new byte[16 * 1024];
            int n;
            while ((n = in.read(buf)) > 0) {
                bos.write(buf, 0, n);
            }
        }
        return bos.toByteArray();
    }
}
