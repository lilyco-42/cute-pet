#!/usr/bin/env bash
#
# cute-pet · Android WebView 悬浮窗壳 —— 构建脚本
#
# 设计取舍: 壳**只用 framework API**(WebView / WindowManager / MediaProjection 等),
# 不引 androidx 与任何第三方库, 所以不需要 Gradle、不需要联网拉依赖 ——
# 用 Android SDK 自带的 aapt2 + javac + d8 + zipalign + apksigner 就能出包。
# 代价: 没有 Gradle 的依赖管理/多渠道/资源合并, 但本壳零资源、零依赖, 够用。
#
# 用法:
#   ./build.sh                          # 出 release APK(商业版 wasm: 不含第三方角色素材)
#   ./build.sh --wasm <path>            # 指定 wasm(如本地 --features bundle-murasame 的测试产物)
#   ./build.sh --out <dir>              # 产物输出目录(默认 ./bin)
#   ./build.sh --skip-wasm              # wasm 已就位时跳过拷贝检查
#
# 产物: <out>/cute-pet-shell.apk
#
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$here/../../.." && pwd)"        # pet/webview/android -> 仓库根
out_dir="$here/bin"
wasm_src="$repo_root/pet/target/wasm32-unknown-unknown/release-wasm/cute-pet.wasm"
skip_wasm=0

while [ $# -gt 0 ]; do
  case "$1" in
    --wasm) wasm_src="$2"; shift 2 ;;
    --out)  out_dir="$2"; shift 2 ;;
    --skip-wasm) skip_wasm=1; shift ;;
    -h|--help) sed -n '2,22p' "$0"; exit 0 ;;
    *) echo "未知参数: $1"; exit 1 ;;
  esac
done

# ---------------- 1. SDK / 工具定位 ----------------

# 坑: Windows 上 ANDROID_HOME 常是 D:\xxx 反斜杠形式, bash 的 glob 认不出来;
# 而且"有 ANDROID_HOME"不等于"装了 build-tools"(scoop 那份就是空的)。
# 所以: 先收集候选 -> 逐个规范化成 POSIX -> 挑第一个同时有 build-tools 和 platform 的。
toposix() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -u "$1" 2>/dev/null || printf '%s' "$1"
  else printf '%s' "$1"; fi
}

cands=()
[ -n "${ANDROID_HOME:-}" ] && cands+=("$(toposix "$ANDROID_HOME")")
[ -n "${ANDROID_SDK_ROOT:-}" ] && cands+=("$(toposix "$ANDROID_SDK_ROOT")")
cands+=("$HOME/Android/Sdk" "$HOME/android_sdk" "${LOCALAPPDATA:-}/Android/Sdk"
        /d/android_sdk /c/android_sdk /usr/local/lib/android/sdk)

# ⚠️ 末尾的 `|| true` 不能省: 脚本开了 pipefail, 候选目录不存在时 ls 失败会让
# 整条管道返回非 0, 赋值语句直接被 set -e 干掉(表现为"脚本静默退出 2")。
pick_latest() { ls -d "$@" 2>/dev/null | sort -V | tail -1 || true; }

SDK=""
for cand in "${cands[@]}"; do
  [ -n "$cand" ] && [ -d "$cand" ] || continue
  b="$(pick_latest "$cand"/build-tools/*)"
  p="$(pick_latest "$cand"/platforms/android-2* "$cand"/platforms/android-3*)"
  if [ -n "$b" ] && [ -n "$p" ]; then
    SDK="$cand"; BT="$b"; PLAT="$p"; break
  fi
done
if [ -z "$SDK" ]; then
  echo "找不到可用的 Android SDK(需要同时有 build-tools 与 platforms)。已尝试:"
  printf '  %s\n' "${cands[@]}"
  echo "请设置 ANDROID_HOME, 或先装: sdkmanager --install 'build-tools;34.0.0' 'platforms;android-34'"
  exit 1
fi

# Git Bash(Windows)下 aapt2/javac/d8 都是原生 Windows 程序, 不认 /d/... 这种
# POSIX 绝对路径 —— 传给它们之前必须转成 D:/... 。Linux(CI)没有 cygpath, 原样返回。
# ⚠️ 两个分支都必须输出换行: 这个函数还被用来生成 javac 的 @argfile,
# 不带换行会把所有 .java 路径粘成一行(javac 报 "file not found: a.javab.javac.java")。
if command -v cygpath >/dev/null 2>&1; then
  winpath() { local out; out="$(cygpath -w "$1")"; printf '%s\n' "$out"; }
else
  winpath() { printf '%s\n' "$1"; }
fi

AAPT2="$BT/aapt2"; [ -x "$AAPT2.exe" ] && AAPT2="$AAPT2.exe"
ZIPALIGN="$BT/zipalign"; [ -x "$ZIPALIGN.exe" ] && ZIPALIGN="$ZIPALIGN.exe"
D8="$BT/d8"; [ -x "$D8.exe" ] && D8="$D8.exe"
APKSIGNER_JAR="$(winpath "$BT/lib/apksigner.jar")"
D8_JAR="$(winpath "$BT/lib/d8.jar")"
ANDROID_JAR="$(winpath "$PLAT/android.jar")"
MANIFEST_ARG="$(winpath "$here/AndroidManifest.xml")"

echo "SDK       : $SDK"
echo "build-tool: $(basename "$BT")"
echo "platform  : $(basename "$PLAT")"

# ---------------- 2. 组装 Web 内容层 ----------------

assets="$out_dir/assets/pet"
mkdir -p "$assets"
cp "$repo_root/pet/webview/web/index.html"     "$assets/index.html"
cp "$repo_root/pet/webview/web/pet_bridge.js"  "$assets/pet_bridge.js"
cp "$repo_root/pages/ply_bundle.js"            "$assets/ply_bundle.js"

if [ "$skip_wasm" -eq 0 ]; then
  [ -f "$wasm_src" ] || {
    echo "wasm 不存在: $wasm_src"
    echo "先构建:  cargo build --target wasm32-unknown-unknown --profile release-wasm"
    exit 1
  }
  cp "$wasm_src" "$assets/app.wasm"
fi
[ -f "$assets/app.wasm" ] || { echo "缺少 app.wasm"; exit 1; }

echo "内容层    : $(du -sh "$assets" | cut -f1) (index.html / pet_bridge.js / ply_bundle.js / app.wasm)"

# ---------------- 3. 编译 ----------------

build="$out_dir/build"
rm -rf "$build"
mkdir -p "$build/obj" "$build/dex" "$build/gen"

# 原生 Windows 工具(aapt2/javac/d8)同样不认 POSIX 绝对路径, 统一给 Windows 形态
BUILD_WIN="$(winpath "$build")"
OBJ_WIN="$(winpath "$build/obj")"
DEX_WIN="$(winpath "$build/dex")"
GEN_WIN="$(winpath "$build/gen")"
APK_WIN="$(winpath "$build/app.unaligned.apk")"

echo "aapt2 link ..."
# res 目录不存在也能 link(本壳零资源, UI 全在代码里搭)
"$AAPT2" link \
  -I "$ANDROID_JAR" \
  --manifest "$MANIFEST_ARG" \
  --min-sdk-version 26 \
  --target-sdk-version 33 \
  --java "$GEN_WIN" \
  -o "$APK_WIN" || {
    echo "aapt2 link 失败"; exit 1; }

echo "javac ..."
# argfile 里也必须是 Windows 路径: javac 会把 /c/... 当成 \c\...(盘符相对路径)
find "$here/src" -name '*.java' | while read -r f; do winpath "$f"; done > "$build/sources.txt"
javac -nowarn -encoding UTF-8 \
  -cp "$ANDROID_JAR" \
  -d "$OBJ_WIN" \
  @"$(winpath "$build/sources.txt")"

echo "d8 ..."
class_args="$(find "$build/obj" -name '*.class' | while read -r f; do winpath "$f"; done | tr '\n' ' ')"
if [ -x "$D8" ]; then
  "$D8" --lib "$ANDROID_JAR" --output "$DEX_WIN" $class_args
else
  java -jar "$D8_JAR" --lib "$ANDROID_JAR" --output "$DEX_WIN" $class_args
fi

echo "打包 assets + dex ..."
# assets: aapt2 link 无法事后追加目录, 这里用 zip 直接塞进 APK 根目录树
(cd "$out_dir/assets" && zip -qr "$APK_WIN" pet)
# dex 必须在 APK 根
(cd "$build/dex" && zip -qj "$APK_WIN" classes.dex)

# ---------------- 4. 对齐 + 签名 ----------------

if [ -x "$ZIPALIGN" ]; then
  echo "zipalign ..."
  "$ZIPALIGN" -f 4 \
    "$(winpath "$build/app.unaligned.apk")" \
    "$(winpath "$build/app.aligned.apk")"
else
  echo "zipalign 不可用, 跳过"
  cp "$build/app.unaligned.apk" "$build/app.aligned.apk"
fi

ks="$out_dir/debug.keystore"
if [ ! -f "$ks" ]; then
  echo "生成调试签名 ..."
  # keytool 也是 Java 程序: 给 POSIX 路径会在 C: 盘根下建出 \c\Users\... 这种怪目录
  keytool -genkeypair -v -keystore "$(winpath "$ks")" -storepass android -keypass android \
    -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=cute-pet Debug,O=cute-pet,C=CN" >/dev/null
fi

echo "apksigner ..."
java -jar "$APKSIGNER_JAR" sign \
  --ks "$(winpath "$ks")" --ks-pass pass:android --key-pass pass:android \
  --out "$(winpath "$out_dir/cute-pet-shell.apk")" "$(winpath "$build/app.aligned.apk")"

echo
echo "✅ 产物: $out_dir/cute-pet-shell.apk  ($(du -h "$out_dir/cute-pet-shell.apk" | cut -f1))"
