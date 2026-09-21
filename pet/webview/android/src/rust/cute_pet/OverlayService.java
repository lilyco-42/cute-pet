package rust.cute_pet;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.PixelFormat;
import android.graphics.Point;
import android.hardware.display.DisplayManager;
import android.os.Build;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.view.Display;
import android.view.Gravity;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowManager;
import android.view.inputmethod.InputMethodManager;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;

import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;

/**
 * 悬浮窗宿主 —— WebView 版(M1)。
 *
 * 与原生版 pet/java/OverlayService.java 的关系: 悬浮窗骨架(权限/前台服务/拖动/
 * 剪贴板/输入法)全部沿用, **唯一的变化是内容区**:
 *   QuadSurface(miniquad 原生渲染表面)  ->  WebView(加载 wasm 内核)。
 *
 * 三个必须处理好的点:
 *  1. 透明: setBackgroundColor(Color.TRANSPARENT) + 页面侧 body/canvas 透明
 *     (Rust 侧 clear_background(0,0,0,0) 已是透明, 见 pet/src/run.rs)。
 *  2. 点击穿透 vs 可交互: 这是悬浮窗的经典矛盾, 本壳做成**可切换**:
 *     - 交互模式(默认): 不加 FLAG_NOT_FOCUSABLE, 窗口收触摸 -> 桌宠可点可拖
 *     - 穿透模式: setPassthrough(true) 加回 FLAG_NOT_FOCUSABLE -> 触摸穿到下层应用
 *     (按角色像素自动切换属于 M2/M3: JS 侧做 alpha 命中测试后调 setPassthrough)
 *  3. 拖动: 触摸监听挂在容器上; 位移超阈值才判定为拖动并吃掉事件,
 *     否则 return false 让 WebView/JS 收到点击。
 */
public class OverlayService extends Service implements PetBridge.Host {

    private static final String TAG = "CutePetOverlay";
    private static final String CHANNEL_ID = "cute_pet_overlay";
    private static final int NOTIF_ID = 1001;

    /** Web 内容层在 APK assets 下的目录: file:///android_asset/pet/index.html */
    private static final String CONTENT_URL = "file:///android_asset/pet/index.html";

    private WindowManager wm;
    private FrameLayout root;
    private WebView webView;
    private WindowManager.LayoutParams params;
    private PetBridge bridge;
    private final Handler main = new Handler(Looper.getMainLooper());

    /** 穿透模式: true = 悬浮窗不吃触摸(下层应用可用) */
    private boolean passthrough = false;

    // ---------------- 生命周期 ----------------

    @Override
    public void onCreate() {
        super.onCreate();
        createChannel();
        startForegroundWithType();
        createWebView();
        setupOverlay();
    }

    /** targetSdk 34(Android 14) 起 startForeground 必须带前台服务类型, 否则抛
     *  MissingForegroundServiceTypeException 直接崩。specialUse 覆盖"常驻浮窗"这一无标准类型可归的用例。 */
    private void startForegroundWithType() {
        android.app.Notification n = buildNotification();
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(NOTIF_ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
        } else {
            startForeground(NOTIF_ID, n);
        }
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (root == null) {
            setupOverlay();
        }
        return START_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    @Override
    public void onDestroy() {
        if (root != null) {
            try {
                wm.removeView(root);
            } catch (Exception ignored) {
            }
        }
        if (webView != null) {
            try {
                webView.destroy();
            } catch (Exception ignored) {
            }
        }
        root = null;
        webView = null;
        super.onDestroy();
    }

    // ---------------- WebView ----------------

    private void createWebView() {
        webView = new WebView(this);

        // 关键 1: 透明。不做这一步就是一个白块/黑块。
        webView.setBackgroundColor(Color.TRANSPARENT);

        WebSettings s = webView.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        // 内容来自 file://(APK assets), 不给这两个权限 fetch("app.wasm") 会被同源策略挡掉
        s.setAllowFileAccess(true);
        s.setAllowFileAccessFromFileURLs(true);
        s.setAllowUniversalAccessFromFileURLs(true);
        s.setMediaPlaybackRequiresUserGesture(false);
        s.setSupportZoom(false);
        s.setBuiltInZoomControls(false);

        webView.setWebViewClient(new WebViewClient() {
            @Override
            public void onPageFinished(WebView view, String url) {
                android.util.Log.i(TAG, "page finished: " + url);
            }
        });
        webView.setWebChromeClient(new WebChromeClient());

        // 关键 2: 原生桥
        bridge = new PetBridge(this);
        webView.addJavascriptInterface(bridge, "PetBridge");

        webView.loadUrl(CONTENT_URL);
    }

    // ---------------- 悬浮窗 ----------------

    private void setupOverlay() {
        if (root != null) {
            return;
        }
        wm = (WindowManager) getSystemService(WINDOW_SERVICE);

        int w = dp(160);
        int h = dp(280);

        params = new WindowManager.LayoutParams(
                w, h,
                WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
                buildFlags(),
                PixelFormat.TRANSLUCENT);
        params.gravity = Gravity.TOP | Gravity.START;
        params.setTitle("cute-pet");

        Point size = new Point();
        Display display = getSystemService(DisplayManager.class).getDisplay(Display.DEFAULT_DISPLAY);
        display.getRealSize(size);
        params.x = size.x - w - dp(8);
        params.y = Math.max(dp(48), (size.y - h) / 2);

        root = new FrameLayout(this) {
            @Override
            protected void onConfigurationChanged(Configuration newConfig) {
                super.onConfigurationChanged(newConfig);
                // 旋转/分屏后窗口可能跑出屏幕, 夹回来
                clampToScreen();
            }
        };
        root.addView(webView,
                new FrameLayout.LayoutParams(
                        FrameLayout.LayoutParams.MATCH_PARENT,
                        FrameLayout.LayoutParams.MATCH_PARENT));
        attachDragHandler();

        try {
            wm.addView(root, params);
        } catch (Exception e) {
            android.util.Log.e(TAG, "addView failed: " + e);
        }
    }

    private int buildFlags() {
        int flags = WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
                | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS;
        if (passthrough) {
            // 不吃触摸 + 不抢焦点 -> 触摸穿透到下层应用
            flags |= WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                    | WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE;
        }
        return flags;
    }

    private void applyFlags() {
        if (params == null || root == null) {
            return;
        }
        params.flags = buildFlags();
        try {
            wm.updateViewLayout(root, params);
        } catch (Exception ignored) {
        }
    }

    /**
     * 拖动: 阈值内不吃事件(交给 WebView/JS 当点击), 超过阈值才吃下并移动窗口。
     */
    private void attachDragHandler() {
        final float[] down = new float[2];
        final int[] win = new int[2];
        final boolean[] dragging = new boolean[1];
        final float slop = dp(10);

        root.setOnTouchListener((v, ev) -> {
            switch (ev.getActionMasked()) {
                case MotionEvent.ACTION_DOWN:
                    down[0] = ev.getRawX();
                    down[1] = ev.getRawY();
                    win[0] = params.x;
                    win[1] = params.y;
                    dragging[0] = false;
                    break;
                case MotionEvent.ACTION_MOVE: {
                    float dx = ev.getRawX() - down[0];
                    float dy = ev.getRawY() - down[1];
                    if (!dragging[0] && Math.hypot(dx, dy) > slop) {
                        dragging[0] = true;
                    }
                    if (dragging[0]) {
                        params.x = win[0] + Math.round(dx);
                        params.y = win[1] + Math.round(dy);
                        try {
                            wm.updateViewLayout(root, params);
                        } catch (Exception ignored) {
                        }
                    }
                    break;
                }
                case MotionEvent.ACTION_UP:
                case MotionEvent.ACTION_CANCEL:
                    if (!dragging[0]) {
                        return false; // 交给 JS: 点击桌宠
                    }
                    break;
            }
            return dragging[0];
        });
    }

    private void clampToScreen() {
        if (params == null || root == null) {
            return;
        }
        Point size = new Point();
        Display display = getSystemService(DisplayManager.class).getDisplay(Display.DEFAULT_DISPLAY);
        display.getRealSize(size);
        params.x = Math.max(0, Math.min(params.x, size.x - params.width));
        params.y = Math.max(0, Math.min(params.y, size.y - params.height));
        try {
            wm.updateViewLayout(root, params);
        } catch (Exception ignored) {
        }
    }

    // ---------------- PetBridge.Host ----------------

    @Override
    public void evalJs(String js) {
        main.post(() -> {
            if (webView != null) {
                webView.evaluateJavascript(js, null);
            }
        });
    }

    @Override
    public void moveBy(int dx, int dy) {
        main.post(() -> {
            if (params == null || root == null) {
                return;
            }
            params.x += dx;
            params.y += dy;
            try {
                wm.updateViewLayout(root, params);
            } catch (Exception ignored) {
            }
        });
    }

    @Override
    public void setSizeDp(int w, int h) {
        main.post(() -> {
            if (params == null || root == null) {
                return;
            }
            params.width = dp(w);
            params.height = dp(h);
            clampToScreen();
        });
    }

    @Override
    public void setPassthrough(boolean on) {
        main.post(() -> {
            passthrough = on;
            applyFlags();
        });
    }

    @Override
    public String getClipboard() {
        ClipboardManager cb = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
        if (cb == null || !cb.hasPrimaryClip()) {
            return null;
        }
        ClipData cd = cb.getPrimaryClip();
        if (cd == null || cd.getItemCount() < 1) {
            return null;
        }
        CharSequence t = cd.getItemAt(0).getText();
        return t == null ? null : t.toString();
    }

    @Override
    public void setClipboard(String text) {
        ClipboardManager cb = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
        if (cb == null) {
            return;
        }
        cb.setPrimaryClip(ClipData.newPlainText("cute-pet", text));
    }

    @Override
    public void showKeyboard(boolean show) {
        main.post(() -> {
            if (webView == null) {
                return;
            }
            InputMethodManager imm =
                    (InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
            if (show) {
                webView.requestFocus();
                imm.showSoftInput(webView, 0);
            } else {
                imm.hideSoftInputFromWindow(webView.getWindowToken(), 0);
            }
        });
    }

    @Override
    public void requestCapture() {
        if (webView == null) {
            pushCaptureError("webView 未就绪");
            return;
        }
        final int w = webView.getWidth();
        final int h = webView.getHeight();
        if (w <= 0 || h <= 0) {
            pushCaptureError("webView 尺寸为 0(尚未布局)");
            return;
        }
        final Bitmap bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888);
        try {
            // WebView 渲染在独立 Surface 上, PixelCopy 的 View 重载在部分平台缺失;
            // 改用软件层 draw —— 对小尺寸桌宠一次性截屏足够, 且全 SDK 可用, 透明通道保留。
            int prev = webView.getLayerType();
            webView.setLayerType(View.LAYER_TYPE_SOFTWARE, null);
            webView.draw(new Canvas(bmp));
            webView.setLayerType(prev, null);
            pushCapture(bmp);
        } catch (Exception e) {
            pushCaptureError(String.valueOf(e));
        }
    }

    /** 把 Bitmap 压成 PNG(base64, 保留透明)经 Bridge 推给 JS: window.PetNative.onMessage({type:"capture.frame"}) */
    private void pushCapture(Bitmap bmp) {
        try {
            int w = bmp.getWidth();
            int h = bmp.getHeight();
            ByteArrayOutputStream bos = new ByteArrayOutputStream();
            bmp.compress(Bitmap.CompressFormat.PNG, 100, bos);
            bmp.recycle();
            String data = android.util.Base64.encodeToString(bos.toByteArray(), android.util.Base64.NO_WRAP);
            JSONObject p = new JSONObject();
            p.put("encoding", "base64");
            p.put("format", "png");
            p.put("width", w);
            p.put("height", h);
            p.put("data", data);
            bridge.push("capture.frame", p);
        } catch (Exception e) {
            pushCaptureError(String.valueOf(e));
        }
    }

    private void pushCaptureError(String msg) {
        try {
            JSONObject p = new JSONObject();
            p.put("error", msg);
            bridge.push("capture.frame", p);
        } catch (Exception ignored) {
        }
        android.util.Log.w(TAG, "capture.request: " + msg);
    }

    @Override
    public File[] assetDirs() {
        // 与 Rust external_asset_dirs() 的 Android 分支保持一致, 用户放一份两边都能用。
        return new File[]{
                new File(getExternalFilesDir(null), "assets"), // /sdcard/Android/data/rust.cute_pet/files/assets
                new File(getFilesDir(), "assets"),             // /data/data/rust.cute_pet/files/assets
                new File("/sdcard/cute-pet/assets"),           // 兜底(新版 Android 可能无权限)
        };
    }

    // ---------------- 常驻通知 ----------------

    private void createChannel() {
        NotificationManager nm = getSystemService(NotificationManager.class);
        NotificationChannel ch = new NotificationChannel(
                CHANNEL_ID, "cute-pet 悬浮窗", NotificationManager.IMPORTANCE_LOW);
        ch.setShowBadge(false);
        nm.createNotificationChannel(ch);
    }

    private Notification buildNotification() {
        Intent i = new Intent(this, MainActivity.class);
        PendingIntent pi = PendingIntent.getActivity(
                this, 0, i,
                Build.VERSION.SDK_INT >= 23 ? PendingIntent.FLAG_IMMUTABLE : 0);
        return new Notification.Builder(this, CHANNEL_ID)
                .setContentTitle("cute-pet")
                .setContentText("悬浮窗运行中 · 拖动换个位置")
                .setSmallIcon(android.R.drawable.ic_menu_compass)
                .setContentIntent(pi)
                .setOngoing(true)
                .build();
    }

    private int dp(int v) {
        return Math.round(v * getResources().getDisplayMetrics().density);
    }
}
