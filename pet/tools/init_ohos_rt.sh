#!/usr/bin/env bash
# 初始化 OHOS 交叉编译 CRT: 把 NDK 的 clang_rt crtbegin/crtend 复制为
# clang 默认查找的 crtbeginS.o/crtendS.o, 放进 sysroot 的架构 lib 目录。
# 幂等: 已存在则跳过。每次 SDK 路径变化/首次构建前运行一次。
set -euo pipefail

SYSROOT="/d/ohos-sdk/command-line-tools/sdk/default/openharmony/native/sysroot"
RTLIB="/d/ohos-sdk/command-line-tools/sdk/default/openharmony/native/llvm/lib/clang/15.0.4/lib/aarch64-linux-ohos"
DEST="$SYSROOT/usr/lib/aarch64-linux-ohos"

mkdir -p "$DEST"

# clang 默认按 crtbeginS.o/crtendS.o 名字在 sysroot lib 搜索
for pair in "clang_rt.crtbegin.o:crtbeginS.o" "clang_rt.crtend.o:crtendS.o"; do
  src="${pair%%:*}"
  dst="${pair##*:}"
  if [ ! -f "$DEST/$dst" ]; then
    cp "$RTLIB/$src" "$DEST/$dst"
    echo "linked $src -> $DEST/$dst"
  else
    echo "$dst 已存在, 跳过"
  fi
done
ls -la "$DEST"/crt*S.o 2>/dev/null || true
echo "CRT 就绪"
