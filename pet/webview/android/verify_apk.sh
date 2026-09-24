#!/usr/bin/env bash
# 壳产物(APK)校验 —— 把"手工解包核验"固化成可重复执行的测试。
#
# 为什么需要它: Java 侧没有单元测试框架(不用 Gradle, 零依赖手搓 aapt2/javac/d8),
# 而壳里真正容易出事的都是**打包完整性**问题 —— 漏 dex 一个类、漏一个 .so、模型少
# 一个文件、误把 180MB 词典或角色素材打进包。这些断言在编译期全是绿的, 只有解包
# 才能发现。本地与 CI 跑的是同一套断言, 避免"CI 里 inline grep 与本地手工核验漂移"。
#
# 用法:
#   ./verify_apk.sh <apk路径> [--selftest]
#     --selftest  自测构建(带角色素材): 跳过 murasame 闸门, 其余照样校验
# 退出码: 0 = 全部通过; 1 = 有断言失败(打印 ERROR 行)
set -uo pipefail

APK="${1:-}"
SELFTEST="${2:-}"

if [ -z "$APK" ] || [ ! -f "$APK" ]; then
  echo "用法: $0 <apk路径> [--selftest]" >&2
  exit 2
fi

FAIL=0
ok()   { echo "  ✅ $1"; }
bad()  { echo "  ❌ $1"; FAIL=1; }

# APK 内条目清单(所有断言共用一次解包, 避免反复读 zip)
ENTRIES="$(unzip -Z1 "$APK" 2>/dev/null)" || {
  echo "❌ 无法解包: $APK"; exit 1; }

echo "== 校验 $(basename "$APK") ($(du -h "$APK" | cut -f1)) =="

# ---------- 1. 原生库: 4 个 .so 缺一不可 ----------
echo "[1/8] 原生库(arm64-v8a)"
for so in libonnxruntime.so libsherpa-onnx-c-api.so libsherpa-onnx-cxx-api.so libsherpa-onnx-jni.so; do
  if printf '%s\n' "$ENTRIES" | grep -qx "lib/arm64-v8a/$so"; then ok "$so"; else bad "缺 $so"; fi
done

# ---------- 2. dex: sherpa 与 kotlin 类都必须在 ----------
# 踩过的坑: 漏把 kotlin-stdlib dex 进去 → 编译期零警告, 运行时
# NoClassDefFoundError: kotlin.jvm.internal.Intrinsics(sherpa Java 层是 Kotlin 编的)。
echo "[2/8] dex 类完整性"
if printf '%s\n' "$ENTRIES" | grep -q '^classes.*\.dex$'; then
  ok "classes.dex 存在"
else
  bad "缺 classes.dex"
fi
DEX_DUMP="$(unzip -p "$APK" classes.dex 2>/dev/null | grep -ac 'com/k2fsa/sherpa/onnx' || true)"
[ "${DEX_DUMP:-0}" -gt 0 ] && ok "dex 含 sherpa-onnx 类" || bad "dex 里找不到 com/k2fsa/sherpa/onnx(桥接会崩)"
DEX_KT="$(unzip -p "$APK" classes.dex 2>/dev/null | grep -ac 'kotlin/jvm/internal' || true)"
[ "${DEX_KT:-0}" -gt 0 ] && ok "dex 含 kotlin-stdlib 类" || bad "dex 里找不到 kotlin/jvm/internal(sherpa Java 层依赖它)"

# ---------- 3. TTS 模型: 5 件基础件 ----------
echo "[3/8] 离线 TTS 模型"
TTS_DIR="pet/tts/vits-icefall-zh-aishell3"
for f in model.onnx lexicon.txt tokens.txt date.fst number.fst; do
  if printf '%s\n' "$ENTRIES" | grep -qx "$TTS_DIR/$f"; then ok "$f"; else bad "缺 $TTS_DIR/$f"; fi
done

# ---------- 4. 大词典绝不进包 + 体积红线 ----------
echo "[4/8] 体积红线"
if printf '%s\n' "$ENTRIES" | grep -q 'rule\.far'; then
  bad "包内出现 rule.far(180MB jieba 词典, 绝不该进 APK)"
else
  ok "无 rule.far"
fi
# 上限 45MB: 当前 42MB(其中 model.onnx 30MB + onnxruntime 22MB 解压量是合理内容)。
# 历史: 曾有 19MB(解压)/~8MB(压缩) 的 vendor/kokoro-zh 死重把包推到 50MB ——
# 收紧上限让这类回流立刻红(50MB > 45MB)。
APK_MB=$(( $(wc -c < "$APK") / 1024 / 1024 ))
if [ "$APK_MB" -le 45 ]; then ok "体积 ${APK_MB}MB ≤ 45MB"; else bad "体积 ${APK_MB}MB 超上限 45MB(是否有死重回流?)"; fi

# ---------- 5. 内容层文件齐 ----------
echo "[5/8] 内容层"
for f in pet/index.html pet/app.wasm pet/pet_bridge.js pet/ply_bundle.js; do
  if printf '%s\n' "$ENTRIES" | grep -qx "$f"; then ok "$f"; else bad "缺 $f"; fi
done
# Launcher 图标: res 编译后以 res/mipmap-*-v4/ic_launcher.png 存在,
# 且二进制 manifest(AXML 字符串池)里应出现 ic_launcher 引用。
if printf '%s\n' "$ENTRIES" | grep -q "res/mipmap-.*ic_launcher\.png" \
   && unzip -p "$APK" AndroidManifest.xml 2>/dev/null | grep -aq "ic_launcher"; then
  ok "launcher 图标(mipmap)已内置"
else
  bad "缺 launcher 图标(@mipmap/ic_launcher)"
fi

# ---------- 6. 死重防线: web TTS 路由已下线, 不许回流 ----------
# 该路由(kokoro-zh)已因音素集不匹配「前半句糊 + 英文口音」+ 需联网拉模型而下线
# (2026-09-23), vendor/ 曾白占 19MB 包体。这里挡住它被误加回来。
echo "[6/8] 死重防线(web TTS 已下线)"
VENDOR_HITS="$(printf '%s\n' "$ENTRIES" | grep -E '^pet/vendor/|kokoro|espeak-ng\.wasm' || true)"
if [ -n "$VENDOR_HITS" ]; then
  bad "包内出现已下线的 web TTS 依赖(vendor/kokoro-zh, 约 19MB 死重):"
  printf '%s\n' "$VENDOR_HITS" | sed 's/^/      /'
else
  ok "无 vendor/kokoro-zh 死重"
fi
if printf '%s\n' "$ENTRIES" | grep -qx 'pet/pet_tts_local.js'; then
  bad "包内出现已下线的 pet_tts_local.js"
else
  ok "无 pet_tts_local.js"
fi

# ---------- 7. 合规闸门: 不得含第三方版权角色素材 ----------
echo "[7/8] 合规闸门"
HITS="$(printf '%s\n' "$ENTRIES" | grep -i 'murasame' || true)"
if [ "$SELFTEST" = "--selftest" ]; then
  echo "  ⚠️  自测模式: 跳过素材闸门(该产物禁止发布)"
elif [ -n "$HITS" ]; then
  bad "含第三方版权角色素材(柚子社《千恋＊万花》), 禁止发布:"
  printf '%s\n' "$HITS" | sed 's/^/      /'
else
  ok "无角色素材(商业构建)"
fi

# ---------- 7. 无历史踩坑: vendor 嵌套 ----------
echo "[8/8] 打包结构"
if printf '%s\n' "$ENTRIES" | grep -q 'vendor/vendor/'; then
  bad "出现 vendor/vendor 嵌套(组装前没清空 assets 目录, 体积会虚高)"
else
  ok "无 vendor 嵌套"
fi

echo
if [ "$FAIL" -eq 0 ]; then
  echo "✅ $(basename "$APK") 全部校验通过"
else
  echo "❌ $(basename "$APK") 存在失败断言(见上方 ❌)"
fi
exit "$FAIL"
