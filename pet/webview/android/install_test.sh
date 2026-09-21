#!/usr/bin/env bash
#
# cute-pet · Android 壳 —— 真机安装 + 悬浮窗验证脚本(M2 用)
#
# 本机 AVD 每次 Boot completed 后约 1 分钟内被宿主回收, 无法验证悬浮窗;
# 透明/帧率/拖动手感**必须真机**。这个脚本把"装包 + 授权 + 拉起 + 验证清单"一键化。
#
# 前置:
#   - 手机打开 开发者选项 + USB 调试, USB 连电脑(或无线 adb)
#   - 本地已拿到 APK(从 CI artifact 下载, 或远端构建后传回)
#
# 用法:
#   ./install_test.sh                         # 默认装 bin/cute-pet-shell.apk
#   ./install_test.sh /path/to/cute-pet-shell.apk
#
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
pkg="rust.cute_pet"
apk="${1:-$here/bin/cute-pet-shell.apk}"

if [ ! -f "$apk" ]; then
  echo "✗ 找不到 APK: $apk"
  echo "  先从 CI artifact 下载, 或把路径作为参数传入。"
  exit 1
fi

# adb 是 Windows 原生程序(Git Bash 下), 不认 /c/... POSIX 路径 -> 转 Windows 形态
if command -v cygpath >/dev/null 2>&1; then
  apk_win="$(cygpath -w "$apk")"
else
  apk_win="$apk"
fi

echo "== 设备 =="
adb devices -l

echo "== 安装 APK =="
adb install -r -t "$apk_win"

echo "== 授权(尽力而为; 部分厂商 ROM 仍需手动在设置里开悬浮窗) =="
# 悬浮窗: appops 直接给(侧载/dev 常用, 部分 OEM 无效)
adb shell appops set "$pkg" SYSTEM_ALERT_WINDOW allow 2>/dev/null || echo "  ⚠ appops 授予悬浮窗失败, 请手动: 设置→应用→cute-pet→悬浮窗 允许"
# 通知(Android 13+)
adb shell appops set "$pkg" POST_NOTIFICATION allow 2>/dev/null || true

echo "== 拉起 MainActivity =="
adb shell am start -n "$pkg/.MainActivity" || echo "  ⚠ am start 失败(可能需先手动点开图标)"

echo ""
echo "==================== 真机验证清单(M2) ===================="
echo " [ ] 通知栏出现 'cute-pet 悬浮窗运行中' 常驻通知(前台服务活着)"
echo " [ ] 屏幕角落出现透明桌宠(无白块/黑块 = 透明生效)"
echo " [ ] 拖动桌宠: 超过阈值能移动, 短按不误触发拖动"
echo " [ ] 点击桌宠: 交互模式下宠物有响应(非穿透)"
echo " [ ] 穿透: JS 调 PetShell.setPassthrough(true) 后, 桌宠下方 App 可正常点击"
echo " [ ] 截屏: JS 调 PetShell.requestCapture() 后, 收到 capture.frame 事件且 data 非空"
echo " [ ] logcat: adb logcat -s CutePetOverlay PetBridge  (看桥日志/报错)"
echo "=========================================================="
echo "排错: 若桌宠不显示, 多半是悬浮窗权限没真正授予 —— 去系统设置手动开一次。"
