package quad_native;

import android.view.Surface;

// Java force the MainActivity class to belong to a specially named package
// this package name is forced to be different for each app
// and Java do not have any way to specify a native symbol lookup name..
// To workaround this - all native callbakcks lives in QuadNative class
public class QuadNative {
    // belongs to MainActivity class
    public native static void activityOnCreate(Object activity);
    public native static void activityOnResume();
    public native static void activityOnPause();
    public native static void activityOnDestroy();

    // belongs to QuadSurface class
    public native static void surfaceOnSurfaceCreated(Surface surface);
    public native static void surfaceOnSurfaceDestroyed(Surface surface);
    public native static void surfaceOnTouch(int id, int phase, float x, float y);
    public native static void surfaceOnSurfaceChanged(Surface surface, int width, int height);
    public native static void surfaceOnKeyDown(int keycode);
    public native static void surfaceOnKeyUp(int keycode);
    public native static void surfaceOnCharacter(int character);

    // W2: 屏幕截帧(JPEG)从 Java 截屏服务喂给 Rust 视觉分析
    public native static void onScreenFrame(byte[] jpeg);

    // 聊天键盘: IME 提交的文本/退格 转发给 Rust 输入框
    public native static void surfaceOnCommitText(String text);
}
