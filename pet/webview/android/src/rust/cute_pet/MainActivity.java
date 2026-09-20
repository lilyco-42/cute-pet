package rust.cute_pet;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.Settings;
import android.view.Gravity;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * 壳的入口: 只负责申请权限 + 起悬浮窗服务。
 *
 * 为什么有个 Activity: SYSTEM_ALERT_WINDOW 必须引导用户去系统设置页授权,
 * 而悬浮窗真正的内容在 OverlayService 里(WebView + wasm)。
 *
 * 注意 UI 是代码里搭的(不引 XML/资源), 这样整个壳可以零 res 目录,
 * aapt2 link 一步就能出包。
 */
public class MainActivity extends Activity {

    private TextView status;
    private static final int REQ_NOTIF = 1;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setGravity(Gravity.CENTER_HORIZONTAL);
        int pad = dp(24);
        root.setPadding(pad, pad, pad, pad);

        TextView title = new TextView(this);
        title.setText("cute-pet · WebView 悬浮窗壳 (M1)");
        title.setTextSize(18f);
        title.setGravity(Gravity.CENTER_HORIZONTAL);

        status = new TextView(this);
        status.setGravity(Gravity.CENTER_HORIZONTAL);
        status.setPadding(0, dp(12), 0, dp(12));

        Button permBtn = new Button(this);
        permBtn.setText("1 · 授权悬浮窗权限");
        permBtn.setOnClickListener(v -> requestOverlayPermission());

        Button startBtn = new Button(this);
        startBtn.setText("2 · 启动悬浮窗");
        startBtn.setOnClickListener(v -> startOverlay());

        Button stopBtn = new Button(this);
        stopBtn.setText("停止悬浮窗");
        stopBtn.setOnClickListener(v -> stopService(new Intent(this, OverlayService.class)));

        root.addView(title);
        root.addView(status);
        root.addView(permBtn);
        root.addView(startBtn);
        root.addView(stopBtn);
        setContentView(root);

        askNotificationPermission();
        refreshStatus();
    }

    @Override
    protected void onResume() {
        super.onResume();
        refreshStatus();
    }

    private void askNotificationPermission() {
        if (Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                        != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS}, REQ_NOTIF);
        }
    }

    private void requestOverlayPermission() {
        if (Build.VERSION.SDK_INT >= 23) {
            Intent i = new Intent(
                    Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                    Uri.parse("package:" + getPackageName()));
            startActivity(i);
        }
    }

    private void startOverlay() {
        if (Build.VERSION.SDK_INT >= 23 && !Settings.canDrawOverlays(this)) {
            requestOverlayPermission();
            refreshStatus();
            return;
        }
        Intent i = new Intent(this, OverlayService.class);
        if (Build.VERSION.SDK_INT >= 26) {
            startForegroundService(i);
        } else {
            startService(i);
        }
        refreshStatus();
    }

    private void refreshStatus() {
        boolean overlay = Build.VERSION.SDK_INT < 23 || Settings.canDrawOverlays(this);
        status.setText("悬浮窗权限: " + (overlay ? "已授予" : "未授予(点步骤 1)")
                + "\n素材目录: " + assetHint());
    }

    /** 提示用户把自备素材放哪(与 Rust external_asset_dirs() 的 Android 分支一致) */
    private String assetHint() {
        java.io.File ext = getExternalFilesDir(null);
        if (ext != null) {
            return new java.io.File(ext, "assets").getAbsolutePath();
        }
        return "(不可用)";
    }

    private int dp(int v) {
        return Math.round(v * getResources().getDisplayMetrics().density);
    }
}
