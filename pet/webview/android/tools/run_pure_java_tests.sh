#!/usr/bin/env bash
# 纯 Java 单测 —— 壳里**不依赖 Android** 的逻辑单元, 直接用 javac + java 在 JVM 上验。
#
# 为什么需要它: 壳不用 Gradle、零依赖手搓 aapt2/javac/d8, 而 android.jar 的方法体全是
# `Stub!` ⇒ 任何 import android.* 的类在 JVM 上跑不了(碰一下就抛)。但只要把纯逻辑抽成
# 不依赖 Android 的类(如 TtsSidFile), 就能零依赖地单测 —— 本脚本就是这条路的入口。
#
# 约定:
#   - 被测类: src/rust/cute_pet/*.java 里**不 import android.** 的类
#   - 断言文件: tools/*Test.java (普通 main + 手写断言, 不引 JUnit, 保持 CI 离线)
# 用法: bash tools/run_pure_java_tests.sh
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
src_dir="$here/../src/rust/cute_pet"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# 只挑不依赖 Android 的单元(否则 javac 会因为缺 android.jar 而失败)
units=()
for f in "$src_dir"/*.java; do
  if ! grep -q '^import android\.' "$f"; then
    units+=("$f")
  fi
done
tests=("$here"/*Test.java)

echo "== 纯 Java 单测 =="
echo "单元: $(for u in "${units[@]}"; do basename "$u"; done | tr '\n' ' ')"
echo "断言: $(for t in "${tests[@]}"; do basename "$t"; done | tr '\n' ' ')"

if [ "${#units[@]}" -eq 0 ] || [ "${#tests[@]}" -eq 0 ]; then
  echo "没有可测单元或断言文件" >&2
  exit 1
fi

mkdir -p "$work/rust/cute_pet"
cp "${units[@]}" "${tests[@]}" "$work/rust/cute_pet/"
# 断言文件与单元同包, 一起编译
javac -encoding UTF-8 -d "$work" "$work"/rust/cute_pet/*.java || {
  echo "javac 失败"; exit 1; }

rc=0
for t in "${tests[@]}"; do
  cls="rust.cute_pet.$(basename "$t" .java)"
  echo "--- $cls"
  java -Dfile.encoding=UTF-8 -cp "$work" "$cls" || rc=1
done
exit "$rc"
