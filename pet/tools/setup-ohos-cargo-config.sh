#!/usr/bin/env bash
# 生成 HarmonyOS(OpenHarmony) 交叉编译的 cargo 配置到 ~/.cargo/config.toml。
#
# 仓库里的 pet/.cargo/config.toml 刻意不含本机 SDK 绝对路径(曾经硬编码
# D:/ohos-sdk/..., 别人 clone 下来 linker 失败且报错指向不明)。本脚本按你机器上
# 实际的 SDK 位置生成配置, 写进用户级 ~/.cargo/config.toml, 不污染仓库。
#
# 用法:
#   export OHOS_SDK_NATIVE=<sdk>/default/openharmony/native
#   ./setup-ohos-cargo-config.sh
#
# 幂等: 重复运行会替换本脚本此前写入的段落(按标记识别), 不动其它内容。
set -euo pipefail

MARKER_BEGIN="# >>> cute-pet ohos cross config (generated) >>>"
MARKER_END="# <<< cute-pet ohos cross config <<<"

if [ -z "${OHOS_SDK_NATIVE:-}" ]; then
    echo "error: 请先设置 OHOS_SDK_NATIVE" >&2
    echo "  export OHOS_SDK_NATIVE=<sdk>/default/openharmony/native" >&2
    exit 1
fi

SYSROOT="$OHOS_SDK_NATIVE/sysroot"
LIBDIR="$SYSROOT/usr/lib/aarch64-linux-ohos"

if [ ! -d "$SYSROOT" ]; then
    echo "error: sysroot 不存在: $SYSROOT" >&2
    exit 1
fi
if [ ! -d "$LIBDIR" ]; then
    echo "warning: 库目录不存在, CRT/libunwind 可能未初始化: $LIBDIR" >&2
    echo "         先跑 tools/init_ohos_rt.sh(见 docs/harmonyos-rust.md)" >&2
fi

CONFIG_DIR="$HOME/.cargo"
CONFIG="$CONFIG_DIR/config.toml"
mkdir -p "$CONFIG_DIR"
[ -f "$CONFIG" ] || : >"$CONFIG"

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

# 1) 剔除旧段落, 保留其余内容
awk -v begin="$MARKER_BEGIN" -v end="$MARKER_END" '
    $0 == begin { skip = 1; next }
    $0 == end   { skip = 0; next }
    !skip       { print }
' "$CONFIG" >"$TMP"

# 2) 追加新段落
{
    cat "$TMP"
    echo
    echo "$MARKER_BEGIN"
    echo "[target.aarch64-unknown-linux-ohos]"
    echo 'linker = "clang"'
    echo "rustflags = ["
    echo '    "-C", "link-arg=--target=aarch64-linux-ohos",'
    echo "    \"-C\", \"link-arg=--sysroot=$SYSROOT\","
    echo '    "-C", "link-arg=-L",'
    echo "    \"-C\", \"link-arg=$LIBDIR\","
    echo "]"
    echo "$MARKER_END"
} | awk 'BEGIN { blank = 0 }
    /^[[:space:]]*$/ { blank++; next }
    { while (blank > 0) { print ""; blank-- } print }
' >"$CONFIG"

echo "已写入 $CONFIG"
echo "  sysroot: $SYSROOT"
echo "  libdir:  $LIBDIR"
echo
echo "下一步: 提供编译器环境变量后即可交叉编译, 例如"
echo "  export CC_aarch64_unknown_linux_ohos=\${CC_aarch64_unknown_linux_ohos:-clang}"
echo "  export AR_aarch64_unknown_linux_ohos=\${AR_aarch64_unknown_linux_ohos:-llvm-ar}"
echo "  export CFLAGS_aarch64_unknown_linux_ohos=--target=aarch64-linux-ohos --sysroot=$SYSROOT"
echo "  cargo build --target aarch64-unknown-linux-ohos --release"
