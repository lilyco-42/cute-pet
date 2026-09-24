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

# --out 传相对路径时(CI 里是 `--out dist-shell`), 后面的
#   (cd "$out_dir/assets" && zip ... "$APK_WIN")
# 会因为 cwd 变了而找不到目标 —— 一律转成绝对路径。
case "$out_dir" in
  /*) ;;
  *) out_dir="$(pwd)/$out_dir" ;;
esac
# Windows 上没有 cygpath 时 --out 可能给的是 D:/xxx 形式, 保持原样即可
case "$wasm_src" in
  /*|[A-Za-z]:[/\\]*) ;;
  *) wasm_src="$(pwd)/$wasm_src" ;;
esac

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

# ⚠️ 壳内**不再分发** WebView 内合成的 web TTS 路由(kokoro-zh)。
# 下线依据(用户实听确认 + 本次复核):
#   1. 该社区包用 espeak 音素集, 与模型期望的 misaki[zh] 不一致 —— 合成「后半句准、
#      前半句糊」且带明显英文口音(pet_tts_local.js 头部有原始记录);
#   2. 它还要联网拉 onnx-community/Kokoro-82M 模型, voices/*.bin 得用户手放 —— 与
#      「单机离线可用」相悖;
#   3. 补齐它需再进包 ~157MB(Kokoro v1.1 multi-lang INT8), 性价比为负。
# 因此 vendor/(18MB espeak-ng.wasm + kokoro.web.js) 不再拷进 APK, pet_tts_local.js
# 也不再随包分发(脚本保留在仓库作归档)。壳内离线中文语音由原生 sherpa-onnx 独家承担,
# 见下方 3.5 节。**别把这两段加回来**: verify_apk.sh 有「不得含 vendor/ 与
# pet_tts_local.js」的断言挡着。

# 组装前清掉旧产物: cp -r 对已存在的目标目录会拷成嵌套(vendor/vendor/...),
# 必须从干净状态组装, 保证可重复构建。
rm -rf "$out_dir/assets"

assets="$out_dir/assets/pet"
mkdir -p "$assets"
cp "$repo_root/pet/webview/web/index.html"     "$assets/index.html"
cp "$repo_root/pet/webview/web/pet_bridge.js"  "$assets/pet_bridge.js"
# 内容层到此为止: index.html + pet_bridge.js + app.wasm(+ ply_bundle.js)。
# **不含** pet_tts_local.js 与 vendor/ —— 见上节下线说明; 壳内语音走原生
# sherpa-onnx, 不经 WebView。
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

# ---- 3.5 壳内离线 TTS(sherpa-onnx)依赖 ----
# AAR/classes.jar + arm64 .so + 模型, 按 third_party/cache 惯例**不进 git**,
# 缺失则拉取(已存在则离线复用)。版本 1.13.8: 已内置 OfflineTtsZipVoiceModelConfig,
# 第二步换 ZipVoice + 丛雨参考音无需升级运行时(参考音走用户素材目录, 合规)。
tp="$here/third_party"
cache="$tp/cache"
mkdir -p "$cache"
aar="$cache/sherpa-onnx-1.13.8.aar"
mtar="$cache/vits-icefall-zh-aishell3.tar.bz2"
kjar="$cache/kotlin-stdlib-2.0.21.jar"
[ -f "$aar" ] || curl -fsSL -o "$aar" \
  "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-1.13.8.aar"
[ -f "$mtar" ] || curl -fsSL -o "$mtar" \
  "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-icefall-zh-aishell3.tar.bz2"
# sherpa-onnx 的 Java 层是 Kotlin 编译的, 运行时依赖 kotlin-stdlib(Intrinsics 等),
# 不一起 dex 上机就是 NoClassDefFoundError —— 这个坑没有编译期报错, 只在运行时炸。
[ -f "$kjar" ] || curl -fsSL -o "$kjar" \
  "https://repo1.maven.org/maven2/org/jetbrains/kotlin/kotlin-stdlib/2.0.21/kotlin-stdlib-2.0.21.jar"

aarx="$build/aar"
mkdir -p "$aarx" "$build/lib/arm64-v8a"
unzip -oq "$aar" -d "$aarx"
cp "$aarx/classes.jar" "$build/classes.jar"
cp "$aarx"/jni/arm64-v8a/*.so "$build/lib/arm64-v8a/"

# 模型挑必需件进 assets(与 TtsEngine.MODEL_DIR 对应)。rule.far 是 180MB 的
# jieba 大词典, **绝不进包** —— 数字/日期读法用 date.fst+number.fst 已够(官方 README 同款)。
mex="$cache/model-extract"
mkdir -p "$mex"
[ -d "$mex/vits-icefall-zh-aishell3" ] || tar -xjf "$mtar" -C "$mex"
mtgt="$assets/tts/vits-icefall-zh-aishell3"
mkdir -p "$mtgt"
cp "$mex/vits-icefall-zh-aishell3/model.onnx" \
   "$mex/vits-icefall-zh-aishell3/lexicon.txt" \
   "$mex/vits-icefall-zh-aishell3/tokens.txt" \
   "$mex/vits-icefall-zh-aishell3/date.fst" \
   "$mex/vits-icefall-zh-aishell3/number.fst" "$mtgt/"
echo "tts       : $mtgt ($(du -sh "$mtgt" | cut -f1))"

# 原生 Windows 工具(aapt2/javac/d8)同样不认 POSIX 绝对路径, 统一给 Windows 形态
BUILD_WIN="$(winpath "$build")"
OBJ_WIN="$(winpath "$build/obj")"
DEX_WIN="$(winpath "$build/dex")"
GEN_WIN="$(winpath "$build/gen")"
APK_WIN="$(winpath "$build/app.unaligned.apk")"

echo "aapt2 link ..."
# res 目录存在时先 compile(含 mipmap/ic_launcher 图标); 不存在也能 link
# (本壳 UI 全在代码里搭, res 只有图标, 属可选)
RES_ARGS=()
if [ -d "$here/res" ] && find "$here/res" -type f | grep -q .; then
  echo "aapt2 compile res ..."
  "$AAPT2" compile --dir "$(winpath "$here/res")" -o "$(winpath "$build/res.zip")" || {
    echo "aapt2 compile res 失败"; exit 1; }
  RES_ARGS+=("$(winpath "$build/res.zip")")
fi
"$AAPT2" link \
  -I "$ANDROID_JAR" \
  --manifest "$MANIFEST_ARG" \
  --min-sdk-version 26 \
  --target-sdk-version 33 \
  --java "$GEN_WIN" \
  "${RES_ARGS[@]}" \
  -o "$APK_WIN" || {
    echo "aapt2 link 失败"; exit 1; }

echo "javac ..."
# argfile 里也必须是 Windows 路径: javac 会把 /c/... 当成 \c\...(盘符相对路径)
find "$here/src" -name '*.java' | while read -r f; do winpath "$f"; done > "$build/sources.txt"
# classpath 分隔符: Windows 的 javac 要 ';', Linux(CI) 要 ':'
if command -v cygpath >/dev/null 2>&1; then CP_SEP=";"; else CP_SEP=":"; fi
CLASSES_JAR="$(winpath "$build/classes.jar")"
KOTLIN_JAR="$(winpath "$kjar")"
javac -nowarn -encoding UTF-8 \
  -cp "$ANDROID_JAR$CP_SEP$CLASSES_JAR" \
  -d "$OBJ_WIN" \
  @"$(winpath "$build/sources.txt")"

echo "d8 ..."
class_args="$(find "$build/obj" -name '*.class' | while read -r f; do winpath "$f"; done | tr '\n' ' ')"
# sherpa-onnx 的 classes.jar 与 kotlin-stdlib 也要一起 dex(不只是当 --lib 引用)
if [ -x "$D8" ]; then
  "$D8" --lib "$ANDROID_JAR" --lib "$CLASSES_JAR" --lib "$KOTLIN_JAR" \
    --output "$DEX_WIN" "$CLASSES_JAR" "$KOTLIN_JAR" $class_args
else
  java -jar "$D8_JAR" --lib "$ANDROID_JAR" --lib "$CLASSES_JAR" --lib "$KOTLIN_JAR" \
    --output "$DEX_WIN" "$CLASSES_JAR" "$KOTLIN_JAR" $class_args
fi

echo "打包 assets + dex + lib ..."
# assets: aapt2 link 无法事后追加目录, 这里用 zip 直接塞进 APK 根目录树
(cd "$out_dir/assets" && zip -qr "$APK_WIN" pet)
# dex 必须在 APK 根
(cd "$build/dex" && zip -qj "$APK_WIN" classes.dex)
# 原生库: lib/<abi>/*.so(System.loadLibrary("sherpa-onnx-jni") 按 ABI 找这里)
(cd "$build" && zip -qr "$APK_WIN" lib)

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

# 签名 keystore 来源(按优先级):
#   1. CUTE_PET_KEYSTORE 环境变量指向的已有 keystore —— CI 走这条: workflow 先把
#      secret(SHELL_KEYSTORE_B64)解码成文件传入, 让本机/CI/历次构建**同签名**,
#      用户覆盖安装无需卸载。密钥本体经 GitHub Secret 传递、不入 git,
#      守住 ".gitignore: keystore 含密钥绝不能入库" 的纪律。
#   2. <out_dir>/debug.keystore 已存在则复用(现状行为; 同一 out_dir 内同签名)。
#   3. 都没有: 现场生成 debug 签名(现状行为)。
ks="$out_dir/debug.keystore"
if [ -n "${CUTE_PET_KEYSTORE:-}" ] && [ -f "$CUTE_PET_KEYSTORE" ]; then
  cp "$CUTE_PET_KEYSTORE" "$ks"
  echo "签名      : 固定 keystore(CUTE_PET_KEYSTORE)"
elif [ ! -f "$ks" ]; then
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
