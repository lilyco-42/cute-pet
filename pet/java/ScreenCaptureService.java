package TARGET_PACKAGE_NAME;

import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.PixelFormat;
import android.hardware.display.DisplayManager;
import android.hardware.display.VirtualDisplay;
import android.media.Image;
import android.media.ImageReader;
import android.media.projection.MediaProjection;
import android.media.projection.MediaProjectionManager;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.util.Log;
import android.view.Display;

import java.io.ByteArrayOutputStream;

import quad_native.QuadNative;

/**
 * W2 视觉链路: MediaProjection 截屏前台服务。
 *
 * MainActivity 完成用户授权(createScreenCaptureIntent)后, 把 resultCode + data
 * 交给本服务; 本服务创建 VirtualDisplay + ImageReader, 每 ~4s 取一帧屏幕,
 * 压缩为 JPEG 后经 QuadNative.onScreenFrame() 交给 Rust 侧(视觉模型分析)。
 *
 * Android 16: 锁屏/来电等场景系统会主动停止 MediaProjection,
 * 本服务注册 Callback.onStop() 统一清理并自停; 下次触发时重新走授权。
 * (注: TARGET_PACKAGE_NAME 由 cargo-quad-apk 构建时替换为真实包名)
 */
public class ScreenCaptureService extends Service {

    private static final String CHANNEL_ID = "cute_pet_capture";
    private static final int NOTIF_ID = 2001;
    private static final int REQUEST_CODE = 0x5150;

    // 截屏目标分辨率(720p 足够 VLM 识别, 省流量省内存)
    private static final int CAPTURE_W = 720;
    private static final int CAPTURE_H = 1280;
    private static final int CAPTURE_DPI = 160;
    // 节流: 两次转发给 Rust 的最小间隔(毫秒)
    // 功耗: 30s 一帧足够视觉分析(按钮/90s 环境触发), 避免持续截屏耗电
    private static final long THROTTLE_MS = 30000;

    private MediaProjection projection;
    private VirtualDisplay vDisplay;
    private ImageReader imageReader;
    private Handler handler;
    private long lastSentAt = 0L;

    @Override
    public void onCreate() {
        super.onCreate();
        createChannel();
        startForeground(NOTIF_ID, buildNotification());
        handler = new Handler(Looper.getMainLooper());
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        if (projection != null) {
            return START_NOT_STICKY; // 已在运行
        }
        int resultCode = intent.getIntExtra("resultCode", Activity.RESULT_CANCELED);
        Intent data = intent.getParcelableExtra("data");
        if (resultCode != Activity.RESULT_OK || data == null) {
            stopSelf();
            return START_NOT_STICKY;
        }
        MediaProjectionManager mpm = (MediaProjectionManager) getSystemService(MEDIA_PROJECTION_SERVICE);
        projection = mpm.getMediaProjection(resultCode, data);
        if (projection == null) {
            stopSelf();
            return START_NOT_STICKY;
        }
        projection.registerCallback(new MediaProjection.Callback() {
            @Override
            public void onStop() {
                Log.i("SAPP", "mediaProjection stopped");
                cleanup();
                stopSelf();
            }
        }, handler);
        setupVirtualDisplay();
        return START_NOT_STICKY;
    }

    private void setupVirtualDisplay() {
        DisplayManager dm = (DisplayManager) getSystemService(DISPLAY_SERVICE);
        Display display = dm.getDisplay(Display.DEFAULT_DISPLAY);

        imageReader = ImageReader.newInstance(CAPTURE_W, CAPTURE_H, PixelFormat.RGBA_8888, 2);
        vDisplay = projection.createVirtualDisplay(
                "cute_pet_capture",
                CAPTURE_W, CAPTURE_H, CAPTURE_DPI,
                DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
                imageReader.getSurface(), null, handler);

        imageReader.setOnImageAvailableListener(new ImageReader.OnImageAvailableListener() {
            @Override
            public void onImageAvailable(ImageReader reader) {
                long now = System.currentTimeMillis();
                if (now - lastSentAt < THROTTLE_MS) {
                    return;
                }
                Image image = null;
                try {
                    image = reader.acquireLatestImage();
                    if (image == null) {
                        return;
                    }
                    Bitmap bmp = imageToBitmap(image);
                    if (bmp != null) {
                        ByteArrayOutputStream out = new ByteArrayOutputStream();
                        bmp.compress(Bitmap.CompressFormat.JPEG, 85, out);
                        byte[] jpeg = out.toByteArray();
                        bmp.recycle();
                        lastSentAt = now;
                        Log.i("SAPP", "captured " + jpeg.length + " bytes");
                        QuadNative.onScreenFrame(jpeg);
                    }
                } catch (Exception e) {
                    Log.e("SAPP", "capture failed: " + e);
                } finally {
                    if (image != null) {
                        image.close();
                    }
                }
            }
        }, handler);
    }

    private Bitmap imageToBitmap(Image image) {
        Image.Plane[] planes = image.getPlanes();
        Image.Plane plane = planes[0];
        int width = image.getWidth();
        int height = image.getHeight();
        int pixelStride = plane.getPixelStride();
        int rowStride = plane.getRowStride();
        int rowPadding = rowStride - pixelStride * width;
        Bitmap bmp = Bitmap.createBitmap(width + rowPadding / pixelStride, height, Bitmap.Config.ARGB_8888);
        bmp.copyPixelsFromBuffer(plane.getBuffer());
        if (rowPadding > 0) {
            Bitmap cropped = Bitmap.createBitmap(bmp, 0, 0, width, height);
            bmp.recycle();
            return cropped;
        }
        return bmp;
    }

    private void cleanup() {
        if (vDisplay != null) {
            vDisplay.release();
            vDisplay = null;
        }
        if (imageReader != null) {
            imageReader.close();
            imageReader = null;
        }
        if (projection != null) {
            projection.stop();
            projection = null;
        }
    }

    // ---- 常驻通知 ----

    private void createChannel() {
        NotificationManager nm = getSystemService(NotificationManager.class);
        NotificationChannel ch = new NotificationChannel(
                CHANNEL_ID, "小丛雨录屏中", NotificationManager.IMPORTANCE_LOW);
        ch.setShowBadge(false);
        nm.createNotificationChannel(ch);
    }

    private Notification buildNotification() {
        Intent i = new Intent(this, MainActivity.class);
        PendingIntent pi = PendingIntent.getActivity(this, REQUEST_CODE, i, PendingIntent.FLAG_IMMUTABLE);
        return new Notification.Builder(this, CHANNEL_ID)
                .setContentTitle("小丛雨")
                .setContentText("正在看你的屏幕…")
                .setSmallIcon(android.R.drawable.ic_menu_camera)
                .setContentIntent(pi)
                .setOngoing(true)
                .build();
    }

    @Override
    public void onDestroy() {
        cleanup();
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
