package TARGET_PACKAGE_NAME;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Intent;
import android.graphics.PixelFormat;
import android.graphics.Point;
import android.hardware.display.DisplayManager;
import android.os.IBinder;
import android.view.Display;
import android.view.Gravity;
import android.view.WindowManager;
import android.view.inputmethod.InputMethodManager;

import quad_native.QuadNative;

/**
 * 悬浮窗宿主 (W1 悬浮窗 spike):
 * 把 miniquad 的 QuadSurface(渲染表面) 挂进 TYPE_APPLICATION_OVERLAY 窗口,
 * 并承担原 MainActivity 的 JNI 职责(activityOnCreate + surface 回调),
 * 让 Rust 桌宠直接渲染在悬浮窗里, 覆盖在全屏游戏/任意应用上方。
 *
 * miniquad 渲染线程由 activityOnCreate -> quad_main() 启动, 之后阻塞等待
 * 第一个 SurfaceCreated 消息; 本服务在 overlay QuadSurface 的 surface 就绪时
 * 提供该消息, 渲染线程即把桌宠画进悬浮窗。
 * (注: 本文件经 cargo-quad-apk 编译, TARGET_PACKAGE_NAME 会被替换为真实包名)
 */
public class OverlayService extends Service {

    private static final String CHANNEL_ID = "cute_pet_overlay";
    private static final int NOTIF_ID = 1001;

    private WindowManager wm;
    private QuadSurface view;
    private WindowManager.LayoutParams params;

    @Override
    public void onCreate() {
        super.onCreate();
        createChannel();
        startForeground(NOTIF_ID, buildNotification());
        // 启动 Rust 桌宠(渲染线程会阻塞等待 overlay surface)
        QuadNative.activityOnCreate(this);
        setupOverlay();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        // 重复启动(用户再次点开 app)时不重复建窗
        if (view == null) {
            setupOverlay();
        }
        return START_STICKY;
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    private void setupOverlay() {
        if (view != null) {
            return;
        }
        wm = (WindowManager) getSystemService(WINDOW_SERVICE);

        int w = dp(140);
        int h = dp(260);

        params = new WindowManager.LayoutParams(
                w, h,
                WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
                WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                        | WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                        | WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
                        | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
                PixelFormat.TRANSLUCENT);
        params.gravity = Gravity.TOP | Gravity.START;

        // 默认停在屏幕右侧居中
        Point size = new Point();
        Display display = getSystemService(DisplayManager.class).getDisplay(Display.DEFAULT_DISPLAY);
        display.getRealSize(size);
        params.x = size.x - w - dp(8);
        params.y = Math.max(dp(48), (size.y - h) / 2);

        view = new QuadSurface(this);
        // 拖动: 移动窗口; 单击(未拖动): 转发给 Rust 桌宠(点击互动/说话)
        view.setQuadDragHandler(new QuadSurface.QuadDragHandler() {
            private int winX;
            private int winY;

            @Override
            public void onDragStart() {
                winX = params.x;
                winY = params.y;
            }

            @Override
            public void onDrag(int dx, int dy) {
                params.x = winX + dx;
                params.y = winY + dy;
                try {
                    wm.updateViewLayout(view, params);
                } catch (Exception ignored) {
                }
            }

            @Override
            public void onDragEnd() {
            }
        });

        try {
            wm.addView(view, params);
        } catch (Exception e) {
            android.util.Log.e("OverlayService", "addView failed: " + e);
        }
    }

    // ---- 以下方法供 miniquad 经 JNI 以 ACTIVITY 为对象反射调用 ----

    /** 悬浮窗模式无全屏概念, 忽略 */
    public void setFullScreen(boolean fullscreen) {
    }

    public void showKeyboard(final boolean show) {
        // Service 无 runOnUiThread, 用主线程 Handler
        new android.os.Handler(android.os.Looper.getMainLooper()).post(new Runnable() {
            @Override
            public void run() {
                InputMethodManager imm = (InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
                if (view == null) {
                    return;
                }
                if (show) {
                    imm.showSoftInput(view, 0);
                } else {
                    imm.hideSoftInputFromWindow(view.getWindowToken(), 0);
                }
            }
        });
    }

    public String getClipboardText() {
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

    public void setClipboardText(String text) {
        ClipboardManager cb = (ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
        if (cb == null) {
            return;
        }
        cb.setPrimaryClip(ClipData.newPlainText("label", text));
    }

    // ---- 常驻通知 ----

    private void createChannel() {
        NotificationManager nm = getSystemService(NotificationManager.class);
        NotificationChannel ch = new NotificationChannel(
                CHANNEL_ID, "小丛雨悬浮窗", NotificationManager.IMPORTANCE_LOW);
        ch.setShowBadge(false);
        nm.createNotificationChannel(ch);
    }

    private Notification buildNotification() {
        Intent i = new Intent(this, MainActivity.class);
        PendingIntent pi = PendingIntent.getActivity(this, 0, i, PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Builder(this, CHANNEL_ID)
                .setContentTitle("小丛雨")
                .setContentText("悬浮窗运行中 · 拖动她换个位置")
                .setSmallIcon(android.R.drawable.ic_menu_compass)
                .setContentIntent(pi)
                .setOngoing(true)
                .build();
    }

    private int dp(int v) {
        return Math.round(v * getResources().getDisplayMetrics().density);
    }

    @Override
    public void onDestroy() {
        if (view != null && view.isAttachedToWindow()) {
            try {
                wm.removeView(view);
            } catch (Exception ignored) {
            }
        }
        view = null;
        QuadNative.activityOnDestroy();
        super.onDestroy();
    }
}
