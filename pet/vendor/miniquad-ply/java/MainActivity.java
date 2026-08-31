package TARGET_PACKAGE_NAME;

import javax.microedition.khronos.egl.EGLConfig;
import javax.microedition.khronos.opengles.GL10;

import android.app.Activity;
import android.os.Bundle;
import android.os.Build;
import android.util.Log;

import android.view.View;
import android.view.Surface;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowManager.LayoutParams;
import android.graphics.SurfaceTexture;
import android.view.TextureView;
import android.view.SurfaceHolder;
import android.view.MotionEvent;
import android.view.KeyEvent;
import android.view.inputmethod.InputMethodManager;

import android.content.Context;
import android.content.Intent;
import android.content.res.Configuration;
import android.content.ClipData;
import android.content.ClipboardManager;

import android.graphics.Color;
import android.graphics.Insets;
import android.graphics.PixelFormat;
import android.graphics.Point;
import android.hardware.display.DisplayManager;
import android.media.projection.MediaProjectionManager;
import android.view.Display;
import android.view.Gravity;
import android.view.WindowManager;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.EditorInfo;
import android.text.Editable;
import android.widget.LinearLayout;

import quad_native.QuadNative;

// note: //% is a special miniquad's pre-processor for plugins
// when there are no plugins - //% whatever will be replaced to an empty string
// before compiling

//% IMPORTS

class QuadSurface
    extends
        TextureView
    implements
        TextureView.SurfaceTextureListener,
        View.OnTouchListener,
        View.OnKeyListener {

    // 悬浮窗拖动回调(OverlayService 设置; 未设置时保持原有点击行为)
    public interface QuadDragHandler {
        void onDragStart();
        void onDrag(int dx, int dy);
        void onDragEnd();
    }

    private QuadDragHandler dragHandler;
    private boolean draggingNow;
    private float dragDownX;
    private float dragDownY;
    private static final float DRAG_THRESHOLD = 16.0f;

    public void setQuadDragHandler(QuadDragHandler handler) {
        this.dragHandler = handler;
    }

    public QuadSurface(Context context){
        super(context);
        // TextureView: 内容合成在窗口自身图层内, 悬浮窗不会被其它应用遮挡
        // (SurfaceView 有独立 SF 层, 在 overlay 窗口里会被压到其它窗口之下)
        setSurfaceTextureListener(this);
        setOpaque(false); // 透明背景, 悬浮窗必需

        setFocusable(true);
        setFocusableInTouchMode(true);
        requestFocus();
        setOnTouchListener(this);
        setOnKeyListener(this);
    }

    private Surface nativeSurface;

    @Override
    public void onSurfaceTextureAvailable(SurfaceTexture st, int width, int height) {
        Log.i("SAPP", "surfaceCreated");
        nativeSurface = new Surface(st);
        QuadNative.surfaceOnSurfaceCreated(nativeSurface);
        QuadNative.surfaceOnSurfaceChanged(nativeSurface, width, height);
    }

    @Override
    public void onSurfaceTextureSizeChanged(SurfaceTexture st, int width, int height) {
        Log.i("SAPP", "surfaceChanged");
        QuadNative.surfaceOnSurfaceChanged(nativeSurface, width, height);
    }

    @Override
    public boolean onSurfaceTextureDestroyed(SurfaceTexture st) {
        Log.i("SAPP", "surfaceDestroyed");
        QuadNative.surfaceOnSurfaceDestroyed(nativeSurface);
        nativeSurface = null;
        return true;
    }

    @Override
    public void onSurfaceTextureUpdated(SurfaceTexture st) {
        // GL 直接渲染进 SurfaceTexture, 无需额外处理
    }

    @Override
    public boolean onTouch(View v, MotionEvent event) {
        // 悬浮窗拖动: 超过阈值视为拖动(移动窗口), 否则走原有点击转发
        if (dragHandler != null) {
            int a = event.getActionMasked();
            if (a == MotionEvent.ACTION_DOWN) {
                dragDownX = event.getRawX();
                dragDownY = event.getRawY();
                draggingNow = false;
            } else if (a == MotionEvent.ACTION_MOVE) {
                float dx = event.getRawX() - dragDownX;
                float dy = event.getRawY() - dragDownY;
                if (!draggingNow && (Math.abs(dx) > DRAG_THRESHOLD || Math.abs(dy) > DRAG_THRESHOLD)) {
                    draggingNow = true;
                    dragHandler.onDragStart();
                }
                if (draggingNow) {
                    dragHandler.onDrag((int) dx, (int) dy);
                    return true;
                }
            } else if (a == MotionEvent.ACTION_UP) {
                if (draggingNow) {
                    draggingNow = false;
                    dragHandler.onDragEnd();
                    return true;
                }
            } else if (a == MotionEvent.ACTION_CANCEL) {
                if (draggingNow) {
                    draggingNow = false;
                    dragHandler.onDragEnd();
                }
                return true;
            }
        }

        int pointerCount = event.getPointerCount();
        int action = event.getActionMasked();

        switch(action) {
        case MotionEvent.ACTION_MOVE: {
            for (int i = 0; i < pointerCount; i++) {
                final int id = event.getPointerId(i);
                final float x = event.getX(i);
                final float y = event.getY(i);
                QuadNative.surfaceOnTouch(id, 0, x, y);
            }
            break;
        }
        case MotionEvent.ACTION_UP: {
            final int id = event.getPointerId(0);
            final float x = event.getX(0);
            final float y = event.getY(0);
            QuadNative.surfaceOnTouch(id, 1, x, y);
            break;
        }
        case MotionEvent.ACTION_DOWN: {
            final int id = event.getPointerId(0);
            final float x = event.getX(0);
            final float y = event.getY(0);
            QuadNative.surfaceOnTouch(id, 2, x, y);
            break;
        }
        case MotionEvent.ACTION_POINTER_UP: {
            final int pointerIndex = event.getActionIndex();
            final int id = event.getPointerId(pointerIndex);
            final float x = event.getX(pointerIndex);
            final float y = event.getY(pointerIndex);
            QuadNative.surfaceOnTouch(id, 1, x, y);
            break;
        }
        case MotionEvent.ACTION_POINTER_DOWN: {
            final int pointerIndex = event.getActionIndex();
            final int id = event.getPointerId(pointerIndex);
            final float x = event.getX(pointerIndex);
            final float y = event.getY(pointerIndex);
            QuadNative.surfaceOnTouch(id, 2, x, y);
            break;
        }
        case MotionEvent.ACTION_CANCEL: {
            for (int i = 0; i < pointerCount; i++) {
                final int id = event.getPointerId(i);
                final float x = event.getX(i);
                final float y = event.getY(i);
                QuadNative.surfaceOnTouch(id, 3, x, y);
            }
            break;
        }
        default:
            break;
        }

        return true;
    }

    // docs says getCharacters are deprecated
    // but somehow on non-latyn input all keyCode and all the relevant fields in the KeyEvent are zeros
    // and only getCharacters has some usefull data
    // 聊天键盘: IME 连接激活时抑制 legacy 字符通道(否则每个字符经两条路径重复进入 Rust)
    static boolean imeActive = false;

    @SuppressWarnings("deprecation")
    @Override
    public boolean onKey(View v, int keyCode, KeyEvent event) {
        // IME 激活时裸键事件不可靠(实测 DEL 的 UP 被 IME 吞掉 → 引擎连删清空),
        // 编辑类输入一律走 InputConnection; 这里只保留非 IME 场景(桌面/无 IME 硬件键盘)
        if (imeActive) {
            return true;
        }
        if (event.getAction() == KeyEvent.ACTION_DOWN && keyCode != 0) {
            QuadNative.surfaceOnKeyDown(keyCode);
        }

        if (event.getAction() == KeyEvent.ACTION_UP && keyCode != 0) {
            QuadNative.surfaceOnKeyUp(keyCode);
        }

        if (event.getAction() == KeyEvent.ACTION_UP || event.getAction() == KeyEvent.ACTION_MULTIPLE) {
            if (!imeActive) {
                int character = event.getUnicodeChar();
                if (character == 0) {
                    String characters = event.getCharacters();
                    if (characters != null && !characters.isEmpty()) {
                        character = characters.charAt(0);
                    }
                }

                if (character != 0) {
                    QuadNative.surfaceOnCharacter(character);
                }
            }
        }

        return true;
    }

    // There is an Android bug when screen is in landscape,
    // the keyboard inset height is reported as 0.
    // This code is a workaround which fixes the bug.
    // See https://groups.google.com/g/android-developers/c/50XcWooqk7I
    // For some reason it only works if placed here and not in the parent layout.
    // 聊天键盘: 声明为文本编辑器, IME 才肯附加到本悬浮窗(默认 false 会被静默拒绝)
    @Override
    public boolean onCheckIsTextEditor() {
        return true;
    }

    // There is an Android bug when screen is in landscape,
    // the keyboard inset height is reported as 0.
    // This code is a workaround which fixes the bug.
    // See https://groups.google.com/g/android-developers/c/50XcWooqk7I
    // For some reason it only works if placed here and not in the parent layout.
    @Override
    public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
        //% QUAD_SURFACE_ON_CREATE_INPUT_CONNECTION

        outAttrs.imeOptions |= EditorInfo.IME_FLAG_NO_FULLSCREEN;
        outAttrs.inputType = android.text.InputType.TYPE_CLASS_TEXT;
        imeActive = true;
        // 聊天键盘: 输入框在 Rust/GL 侧渲染, Java 侧不维护真实文本。
        // 不能用 BaseInputConnection —— 它内部的 Editable 与 Rust 缓冲区脱节,
        // IME 会基于假文本反复下发"纠正删除"(表现为打完字自动一个个删掉)。
        return new KBInputConnection();
    }

    /** 最小化 InputConnection: 所有编辑操作直接转发给 Rust, 不维护任何文本状态。
     *  组合词(拼音)期间: setComposingText 实时转发 + 记录长度, 后续用 DEL 抵消重写,
     *  保证 Rust 缓冲区 == 用户所见。 */
    class KBInputConnection extends android.view.inputmethod.InputConnectionWrapper {
        private String composing = "";
        // Rust 侧输入框文本的 Java 镜像: 让 IME 查询到真实文本,
        // 否则 IME 以为缓冲区为空, 退格键不下发 deleteSurroundingText
        private final StringBuilder mirror = new StringBuilder();

        KBInputConnection() {
            super(null, true);
        }

        private void sendDel(int n) {
            for (int i = 0; i < n; i++) {
                QuadNative.surfaceOnKeyDown(android.view.KeyEvent.KEYCODE_DEL);
                QuadNative.surfaceOnKeyUp(android.view.KeyEvent.KEYCODE_DEL);
            }
        }

        @Override
        public boolean setComposingText(CharSequence text, int newCursorPosition) {
            // 组合词(拼音)期间: 不把半成品推给 Rust, 避免 DEL+commit 跨帧交错导致
            // 缓冲区乱序/只剩单字(issue#1)。只记录 composing, 由 commitText 一次性替换。
            Log.i("SAPP", "ic.setComposingText: " + text);
            composing = text == null ? "" : text.toString();
            return true;
        }

        @Override
        public boolean finishComposingText() {
            composing = "";
            return true;
        }

        @Override
        public boolean commitText(CharSequence text, int newCursorPosition) {
            Log.i("SAPP", "ic.commitText: " + text);
            composing = "";
            if (text != null && text.length() > 0) {
                // 整段一次性交给 Rust 插入(单条 TextCommit → 逐字符有序插入),
                // 不再用 sendDel 抵消组合词, 避免跨帧交错乱序(issue#1)。
                QuadNative.surfaceOnCommitText(text.toString());
                mirror.append(text);
            }
            return true;
        }

        @Override
        public boolean deleteSurroundingText(int beforeLength, int afterLength) {
            Log.i("SAPP", "ic.deleteSurroundingText(" + beforeLength + "," + afterLength + ")");
            int n = Math.max(beforeLength, 0);
            if (composing.length() > 0) {
                // 组合词本身已在 Java 侧(没进 Rust), 删 composing 只需减短, 不发 DEL
                int c = Math.min(n, composing.length());
                composing = composing.substring(0, composing.length() - c);
                n -= c;
            }
            if (n > 0) {
                sendDel(n);
                int rm = Math.min(n, mirror.length());
                mirror.delete(mirror.length() - rm, mirror.length());
            }
            return true;
        }

        @Override
        public CharSequence getTextBeforeCursor(int length, int flags) {
            int n = Math.min(Math.max(length, 0), mirror.length());
            return mirror.substring(mirror.length() - n);
        }

        @Override
        public boolean sendKeyEvent(KeyEvent event) {
            int keyCode = event.getKeyCode();
            if (event.getAction() == KeyEvent.ACTION_DOWN && keyCode != 0) {
                QuadNative.surfaceOnKeyDown(keyCode);
            }
            if (event.getAction() == KeyEvent.ACTION_UP && keyCode != 0) {
                QuadNative.surfaceOnKeyUp(keyCode);
            }
            if (event.getAction() == KeyEvent.ACTION_UP || event.getAction() == KeyEvent.ACTION_MULTIPLE) {
                int character = event.getUnicodeChar();
                if (character != 0) {
                    QuadNative.surfaceOnCharacter(character);
                }
            }
            return true;
        }

        @Override
        public boolean performEditorAction(int actionCode) {
            Log.i("SAPP", "ic.performEditorAction: " + actionCode);
            QuadNative.surfaceOnKeyDown(android.view.KeyEvent.KEYCODE_ENTER);
            QuadNative.surfaceOnKeyUp(android.view.KeyEvent.KEYCODE_ENTER);
            return true;
        }

        // ---- 只读查询: 光标后/选区一律空, 避免 IME 基于假文本做"纠正" ----
        @Override
        public CharSequence getTextAfterCursor(int length, int flags) { return ""; }

        @Override
        public CharSequence getSelectedText(int flags) { return null; }

        @Override
        public int getCursorCapsMode(int reqModes) { return 0; }

        @Override
        public android.view.inputmethod.ExtractedText getExtractedText(
                android.view.inputmethod.ExtractedTextRequest request, int flags) { return null; }

        @Override
        public boolean setComposingRegion(int start, int end) { return true; }

        @Override
        public boolean deleteSurroundingTextInCodePoints(int beforeLength, int afterLength) {
            return deleteSurroundingText(beforeLength, afterLength);
        }

        @Override
        public boolean beginBatchEdit() { return true; }

        @Override
        public boolean endBatchEdit() { return true; }

        @Override
        public boolean clearMetaKeyStates(int states) { return true; }

        @Override
        public void closeConnection() {
            imeActive = false;
            super.closeConnection();
        }

        @Override
        public boolean requestCursorUpdates(int cursorUpdateMode) { return false; }

        @Override
        public boolean performPrivateCommand(String action, Bundle data) { return true; }

        // ---- 以下 IME 偶发调用, 安全空实现(避免 wrapper 空目标 NPE) ----
        @Override
        public android.os.Handler getHandler() { return null; }

        @Override
        public boolean commitCompletion(android.view.inputmethod.CompletionInfo info) { return true; }

        @Override
        public boolean commitCorrection(android.view.inputmethod.CorrectionInfo info) { return true; }

        @Override
        public boolean commitContent(android.view.inputmethod.InputContentInfo info, int flags, Bundle opts) { return false; }

        @Override
        public boolean performContextMenuAction(int id) { return true; }

        @Override
        public boolean reportFullscreenMode(boolean enabled) { return true; }

        @Override
        public boolean setSelection(int start, int end) { return true; }

        // ---- API 31+ default 方法: wrapper 会委托空目标导致 NPE, 全部安全覆盖 ----
        @Override
        public android.view.inputmethod.SurroundingText getSurroundingText(int beforeLength, int afterLength, int flags) {
            return null;
        }

        @Override
        public boolean commitText(CharSequence text, int newCursorPosition,
                android.view.inputmethod.TextAttribute textAttribute) {
            return commitText(text, newCursorPosition);
        }

        @Override
        public boolean setComposingText(CharSequence text, int newCursorPosition,
                android.view.inputmethod.TextAttribute textAttribute) {
            return setComposingText(text, newCursorPosition);
        }

        @Override
        public boolean setComposingRegion(int start, int end,
                android.view.inputmethod.TextAttribute textAttribute) {
            return true;
        }

        @Override
        public boolean replaceText(int start, int end, CharSequence text, int newCursorPosition,
                android.view.inputmethod.TextAttribute textAttribute) {
            return commitText(text, newCursorPosition);
        }

        @Override
        public void performHandwritingGesture(android.view.inputmethod.HandwritingGesture gesture,
                java.util.concurrent.Executor executor, java.util.function.IntConsumer consumer) {
        }

        @Override
        public boolean previewHandwritingGesture(
                android.view.inputmethod.PreviewableHandwritingGesture gesture,
                android.os.CancellationSignal cancellationSignal) { return false; }

        @Override
        public boolean requestCursorUpdates(int cursorUpdateMode, int cursorUpdateFilter) { return false; }

        @Override
        public void requestTextBoundsInfo(android.graphics.RectF bounds,
                java.util.concurrent.Executor executor,
                java.util.function.Consumer<android.view.inputmethod.TextBoundsInfoResult> consumer) {
        }

        @Override
        public boolean performSpellCheck() { return true; }

        @Override
        public boolean setImeConsumesInput(boolean imeConsumesInput) { return true; }

        @Override
        public android.view.inputmethod.TextSnapshot takeSnapshot() { return null; }
    }

    public Surface getNativeSurface() {
        SurfaceTexture st = getSurfaceTexture();
        return st == null ? null : new Surface(st);
    }
}

class ResizingLayout
    extends
        LinearLayout
    implements
        View.OnApplyWindowInsetsListener {
    //% RESIZING_LAYOUT_BODY

    public ResizingLayout(MainActivity activity){
        super(activity);
        // When viewing in landscape mode with keyboard shown, there are
        // gaps on both sides so we fill the negative space with black.
        setBackgroundColor(Color.BLACK);
        setOnApplyWindowInsetsListener(this);

        //% RESIZING_LAYOUT_CONSTRUCTOR
    }

    @Override
    public WindowInsets onApplyWindowInsets(View v, WindowInsets insets) {
        // This handler provides a default impl which resizes content when the
        // IME is shown or hidden.
        //
        // However this will lead to flickers in your app as the content is
        // resized before you have a chance to redraw it.
        //
        // The workaround for now is:
        // * Get IME and system insets and send them to your app.
        // * Notify your app to draw and apply the insets to fit.
        // * Disable this function by returning early.

        //% RESIZING_LAYOUT_ON_APPLY_WINDOW_INSETS

        if (Build.VERSION.SDK_INT >= 30) {
            Insets imeInsets = insets.getInsets(WindowInsets.Type.ime());
            Insets sysInsets = insets.getInsets(WindowInsets.Type.systemBars());

            // When IME is visible then we dont need bottom inset
            int bottomPadding = sysInsets.bottom;
            if (imeInsets.bottom > 0) {
                bottomPadding = imeInsets.bottom;
            }

            // The sys insets change when orientation changes and sys bars
            // change position.
            v.setPadding(
                sysInsets.left,
                sysInsets.top,
                sysInsets.right,
                bottomPadding
            );
        }
        return insets;
    }
}

public class MainActivity extends Activity {
    //% MAIN_ACTIVITY_BODY

    // 悬浮窗模式: 由 OverlayService 托管渲染; 置位后本 Activity 跳过自身
    // surface 与生命周期 JNI 调用, 避免 Activity 销毁时杀掉渲染线程。
    public static volatile boolean overlayActive = false;

    // W2: MediaProjection 截屏授权请求码
    private static final int REQUEST_MEDIA_PROJECTION = 4242;

    // W3: 悬浮窗权限引导(首次启动未授权时弹窗去系统设置)
    private static boolean overlayGuideShown = false;

    private QuadSurface view;

    // 悬浮窗窗口句柄(键盘/拖动共用)
    private WindowManager overlayWm;
    private WindowManager.LayoutParams overlayLp;
    private int overlayYBeforeKb;   // 键盘弹出前的窗口 y(收起时还原)

    // 聊天键盘: IME 延迟重试(Operit 模式) —— 窗口变可聚焦后系统需时间处理,
    // 立即 showSoftInput 会因焦点宿主未就绪而静默失败
    private final android.os.Handler mainHandler = new android.os.Handler(android.os.Looper.getMainLooper());
    private Runnable pendingImeShow;
    private static final long IME_FOCUS_DELAY_MS = 200;
    private static final long IME_FOCUS_RETRY_DELAY_MS = 50;
    private static final int MAX_IME_FOCUS_RETRIES = 4;

    static {
        System.loadLibrary("LIBRARY_NAME");
    }

    @Override
    public void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        // 悬浮窗优先: 有 SYSTEM_ALERT_WINDOW 权限就直接以悬浮桌宠启动
        // (Activity 直接托管悬浮窗窗口, 不依赖 Service/manifest 声明)
        if (android.provider.Settings.canDrawOverlays(this)) {
            overlayActive = true;
            startOverlay();
            return;
        }

        // W3: 未授权悬浮窗 → 首次启动弹窗引导去系统设置
        maybeShowOverlayGuide();

        this.requestWindowFeature(Window.FEATURE_NO_TITLE);

        view = new QuadSurface(this);
        // Put it inside a parent layout which can resize it using padding
        ResizingLayout layout = new ResizingLayout(this);
        layout.addView(view);
        setContentView(layout);

        QuadNative.activityOnCreate(this);

        //% MAIN_ACTIVITY_ON_CREATE
    }

    /** 悬浮窗托管: 把渲染表面 QuadSurface 挂进 TYPE_APPLICATION_OVERLAY 窗口。
     *  本 Activity 不 setContentView(不留自己的渲染窗口), 渲染线程由
     *  overlay 窗口的 QuadSurface surface 驱动; Activity 保持存活在后台。 */
    private void startOverlay() {
        QuadNative.activityOnCreate(this);

        overlayWm = (WindowManager) getSystemService(WINDOW_SERVICE);
        final float density = getResources().getDisplayMetrics().density;
        final int w = Math.round(140 * density);
        final int h = Math.round(260 * density);

        overlayLp = new WindowManager.LayoutParams(
                w, h,
                WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                        | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                        | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
                        | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
                PixelFormat.TRANSLUCENT);
        overlayLp.gravity = Gravity.TOP | Gravity.START;

        Point size = new Point();
        getSystemService(DisplayManager.class).getDisplay(Display.DEFAULT_DISPLAY).getRealSize(size);
        overlayLp.x = size.x - w - Math.round(8 * density);
        overlayLp.y = Math.max(Math.round(48 * density), (size.y - h) / 2);

        view = new QuadSurface(this);
        // 聊天键盘: 悬浮窗要能取得焦点, IME 才能附加(TextureView 默认不可聚焦)
        view.setFocusable(View.FOCUSABLE);
        view.setFocusableInTouchMode(true);
        // 拖动: 移动窗口; 单击(未拖动): 转发给 Rust 桌宠(点击互动/说话)
        view.setQuadDragHandler(new QuadSurface.QuadDragHandler() {
            private int winX;
            private int winY;

            @Override
            public void onDragStart() {
                winX = overlayLp.x;
                winY = overlayLp.y;
            }

            @Override
            public void onDrag(int dx, int dy) {
                overlayLp.x = winX + dx;
                overlayLp.y = winY + dy;
                try {
                    overlayWm.updateViewLayout(view, overlayLp);
                } catch (Exception ignored) {
                }
            }

            @Override
            public void onDragEnd() {
            }
        });

        try {
            overlayWm.addView(view, overlayLp);
        } catch (Exception e) {
            Log.e("SAPP", "overlay addView failed: " + e);
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (overlayActive) return;
        // W3: 刚从系统设置开启悬浮窗权限 → 提示手动重启进入悬浮窗
        // (不能同进程自动重启: 旧渲染/音频线程未完全退出会竞争导致 panic)
        if (android.provider.Settings.canDrawOverlays(this) && overlayGuideShown) {
            overlayGuideShown = false;
            android.widget.Toast.makeText(this, "悬浮窗权限已开启，请退出并重新打开小丛雨", android.widget.Toast.LENGTH_LONG).show();
            return;
        }
        QuadNative.activityOnResume();

        //% MAIN_ACTIVITY_ON_RESUME
    }

    /** W3: 悬浮窗权限引导: 弹窗 → 系统悬浮窗设置页。 */
    private void maybeShowOverlayGuide() {
        if (overlayGuideShown || android.provider.Settings.canDrawOverlays(this)) {
            return;
        }
        overlayGuideShown = true;
        try {
            new android.app.AlertDialog.Builder(this)
                    .setTitle("开启悬浮窗权限")
                    .setMessage("让小丛雨浮在游戏/应用上方，需要开启「悬浮窗」权限。")
                    .setPositiveButton("去设置", new android.content.DialogInterface.OnClickListener() {
                        @Override
                        public void onClick(android.content.DialogInterface d, int which) {
                            try {
                                Intent i = new Intent(
                                        android.provider.Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                                        android.net.Uri.parse("package:" + getPackageName()));
                                startActivity(i);
                            } catch (Exception e) {
                                Log.e("SAPP", "overlay settings failed: " + e);
                            }
                        }
                    })
                    .setNegativeButton("暂不", null)
                    .show();
        } catch (Exception e) {
            Log.e("SAPP", "guide dialog failed: " + e);
        }
    }

    @Override
    public void onBackPressed() {
        Log.w("SAPP", "onBackPressed");

        // TODO: here is the place to handle request_quit/order_quit/cancel_quit

        super.onBackPressed();
    }

    @Override
    protected void onStop() {
        super.onStop();
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        if (overlayActive) return;
        QuadNative.activityOnDestroy();
    }

    @Override
    protected void onPause() {
        super.onPause();
        if (overlayActive) return;
        QuadNative.activityOnPause();

        //% MAIN_ACTIVITY_ON_PAUSE
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        // W2: MediaProjection 授权结果 → 启动截屏前台服务
        if (requestCode == REQUEST_MEDIA_PROJECTION && resultCode == Activity.RESULT_OK && data != null) {
            Intent svc = new Intent(this, ScreenCaptureService.class);
            svc.putExtra("resultCode", resultCode);
            svc.putExtra("data", data);
            if (Build.VERSION.SDK_INT >= 26) {
                startForegroundService(svc);
            } else {
                startService(svc);
            }
        }
        //% MAIN_ACTIVITY_ON_ACTIVITY_RESULT
    }

    /** W2: 请求 MediaProjection 授权(由 Rust 经 JNI 触发); 结果走 onActivityResult。 */
    public void startScreenCapture() {
        runOnUiThread(new Runnable() {
            @Override
            public void run() {
                try {
                    MediaProjectionManager mpm = (MediaProjectionManager) getSystemService(MEDIA_PROJECTION_SERVICE);
                    startActivityForResult(mpm.createScreenCaptureIntent(), REQUEST_MEDIA_PROJECTION);
                } catch (Exception e) {
                    Log.e("SAPP", "startScreenCapture failed: " + e);
                }
            }
        });
    }

    public void setFullScreen(final boolean fullscreen) {
        runOnUiThread(new Runnable() {
                @Override
                public void run() {
                    View decorView = getWindow().getDecorView();

                    if (fullscreen) {
                        getWindow().setFlags(LayoutParams.FLAG_LAYOUT_NO_LIMITS, LayoutParams.FLAG_LAYOUT_NO_LIMITS);
                        if (Build.VERSION.SDK_INT >= 28) {
                            getWindow().getAttributes().layoutInDisplayCutoutMode = LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
                        }
                        if (Build.VERSION.SDK_INT >= 30) {
                            getWindow().setDecorFitsSystemWindows(false);
                        } else {
                            int uiOptions = View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY;
                            decorView.setSystemUiVisibility(uiOptions);
                        }
                    }
                    else {
                        if (Build.VERSION.SDK_INT >= 30) {
                            getWindow().setDecorFitsSystemWindows(true);
                        } else {
                          decorView.setSystemUiVisibility(0);
                        }

                    }
                }
            });
    }

    public void showKeyboard(final boolean show) {
        Log.i("SAPP", "showKeyboard(" + show + ")");
        runOnUiThread(new Runnable() {
                @Override
                public void run() {
                    if (overlayWm == null || overlayLp == null || view == null) {
                        return;
                    }
                    InputMethodManager imm = (InputMethodManager) getSystemService(Context.INPUT_METHOD_SERVICE);
                    if (show) {
                        // 悬浮窗取得焦点以便 IME 附加(FLAG_NOT_FOCUSABLE 会拒绝输入法)
                        overlayLp.flags &= ~WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE;
                        try {
                            overlayWm.updateViewLayout(view, overlayLp);
                        } catch (Exception ignored) {
                        }
                        // 键盘弹出时把窗口上移, 避免输入框被键盘盖住
                        overlayYBeforeKb = overlayLp.y;
                        int kbH = Math.round(getResources().getDisplayMetrics().heightPixels * 0.35f);
                        overlayLp.y = Math.max(Math.round(24 * getResources().getDisplayMetrics().density), overlayYBeforeKb - kbH);
                        try {
                            overlayWm.updateViewLayout(view, overlayLp);
                        } catch (Exception ignored) {
                        }
                        // Operit 模式: 窗口变可聚焦后系统需要时间处理,
                        // 等 attach + 焦点宿主就绪后再 showSoftInput(立即调用会静默失败)
                        scheduleImeShow(imm, 0, IME_FOCUS_DELAY_MS);
                    } else {
                        if (pendingImeShow != null) {
                            mainHandler.removeCallbacks(pendingImeShow);
                            pendingImeShow = null;
                        }
                        imm.hideSoftInputFromWindow(view.getWindowToken(), 0);
                        view.clearFocus();
                        QuadSurface.imeActive = false;
                        // 还原窗口位置 + 恢复 NOT_FOCUSABLE(不抢焦点)
                        overlayLp.y = overlayYBeforeKb;
                        overlayLp.flags |= WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE;
                        try {
                            overlayWm.updateViewLayout(view, overlayLp);
                        } catch (Exception ignored) {
                        }
                    }
                }
            });
    }

    private void scheduleImeShow(final InputMethodManager imm, final int retry, final long delayMs) {
        if (pendingImeShow != null) {
            mainHandler.removeCallbacks(pendingImeShow);
        }
        pendingImeShow = new Runnable() {
                @Override
                public void run() {
                    pendingImeShow = null;
                    if (view == null || !view.isAttachedToWindow() || view.getWindowToken() == null) {
                        if (retry < MAX_IME_FOCUS_RETRIES) {
                            scheduleImeShow(imm, retry + 1, IME_FOCUS_RETRY_DELAY_MS);
                        } else {
                            Log.i("SAPP", "ime skip: not attached after retries");
                        }
                        return;
                    }
                    view.requestFocus();
                    View host = view.findFocus();
                    boolean ready = host != null && host.isAttachedToWindow()
                            && host.getWindowToken() != null && host.onCheckIsTextEditor();
                    if (!ready) {
                        if (retry < MAX_IME_FOCUS_RETRIES) {
                            scheduleImeShow(imm, retry + 1, IME_FOCUS_RETRY_DELAY_MS);
                        } else {
                            Log.i("SAPP", "ime skip: no focus host after retries");
                        }
                        return;
                    }
                    Log.i("SAPP", "ime showSoftInput");
                    imm.showSoftInput(host, InputMethodManager.SHOW_IMPLICIT);
                }
            };
        mainHandler.postDelayed(pendingImeShow, delayMs);
    }

    public String getClipboardText() {
        ClipboardManager clipboard = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);

        if (!clipboard.hasPrimaryClip())
            return null;

        ClipData primaryClip = clipboard.getPrimaryClip();
        if (primaryClip == null || primaryClip.getItemCount() < 1)
            return null;

        CharSequence clipData = clipboard.getPrimaryClip().getItemAt(0).getText();
        if (clipData == null) {
            return null;
        }

        return clipData.toString();
    }
    public void setClipboardText(String text) {
        ClipboardManager clipboard = (ClipboardManager) getSystemService(Context.CLIPBOARD_SERVICE);
        ClipData clip = ClipData.newPlainText("label", text);
        clipboard.setPrimaryClip(clip);
    }
}

