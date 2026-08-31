# Rust 开发 HarmonyOS(OpenHarmony) 桌宠: 问题与解决方案

> 为 cute-pet 桌宠(宏 quad + miniquad-ply)添加 HarmonyOS 原生支持时踩的坑与解法。
> 目标: 让 Rust 写的 GDI/EGL 桌宠在鸿蒙 PC/手机上以原生 .so/.elf 运行。

## 总览

- 鸿蒙 Rust 是官方支持(tier 2)的: `aarch64-unknown-linux-ohos` / `x86_64-unknown-linux-ohos`
  / `armv7-unknown-linux-ohos`, 见 [Rust 官方 openharmony.md](https://android.googlesource.com/toolchain/rustc/+/refs/heads/master/src/doc/rustc/src/platform-support/openharmony.md)
- **关键陷阱: 鸿蒙靶的 `target_os` 实际是 `"linux"`, 特色在 `target_env = "ohos"`!**
  `rustc --print cfg --target aarch64-unknown-linux-ohos` 输出:
  ```
  target_os="linux"
  target_env="ohos"
  ```
  所以所有 `#[cfg(target_os="linux")]` 分支在鸿蒙下都会误触发 → 大量 Linux 专属代码(X11/
  wayland/alsa/fontconfig)被编译并尝试链接不存在的系统库。

## 环境搭建

### 1. 工具链
```bash
rustup target add aarch64-unknown-linux-ohos
# 编译器: 任意带 clang 的 LLVM(scoop install llvm)。鸿蒙 NDK 的 clang 15 在 SDK 内
# SDK: Command Line Tools(含 native SDK)解压到某处, 见文末路径
```

### 2. `.cargo/config.toml`(linker + sysroot)
```toml
[target.aarch64-unknown-linux-ohos]
linker = "clang"
rustflags = [
  "-C", "link-arg=--target=aarch64-linux-ohos",
  "-C", "link-arg=--sysroot=D:/ohos-sdk/.../native/sysroot",
  "-C", "link-arg=-L",
  "-C", "link-arg=D:/ohos-sdk/.../native/sysroot/usr/lib/aarch64-linux-ohos",
]
[env]
CC_aarch64_unknown_linux_ohos = ".../clang.exe"
AR_aarch64_unknown_linux_ohos = ".../llvm-ar.exe"
CFLAGS_aarch64_unknown_linux_ohos = "--target=aarch64-linux-ohos --sysroot=D:/.../sysroot"
```

## 遇到的坑与解法

### 坑1: miniquad-ply 无鸿蒙后端
- **现象**: 宏 quad 不支持 ohos, 纯 Rust 也无法链接
- **方案**: vendor `miniquad-ply` 本地(patch.crates-io), 仿 `native/android.rs` 写
  `native/ohos.rs`: EGL 渲染线程 + `ohos_surface_created/changed/destroyed`/
  `ohos_touch`/`ohos_char`/`ohos_pause`/`ohos_resume` 导出符号(供 NAPI 调用)
- `egl.rs` 加 ohos 的 `EGLNativeWindowType = *mut c_void`; `module.rs` 加 ohos
  到 `pub mod linux`(鸿蒙用 dlopen 加载 libEGL); `lib.rs` run 分发加 ohos

### 坑2: `target_os="linux"` 误触发
- **现象**: linker 报 `unable to find library -lX11` / `-lasound`
- **根因**: 鸿蒙 `target_os=linux` 触发 miniquad-ply 的 linux_x11/wayland、quad-snd 的
  alsa、以及本项目自定义 `src/linux.rs`(X11 置顶)
- **方案**: 所有 Linux 专属 cfg 改为排除 ohos:
  ```rust
  #[cfg(all(target_os = "linux", not(target_env = "ohos")))]
  ```
  涉及: `miniquad-ply` 的 native.rs/lib.rs/egl.rs/module.rs + 本项目 `src/linux.rs`

### 坑3: 音频 quad-snd 在鸿蒙走 alsa(-lasound)
- **现象**: `-lasound` 找不到
- **方案**: vendor `quad-snd`, 加 `ohos_snd.rs` noop 桩(结构对齐 alsa_snd: 保留
  `mixer_ctrl` 字段 + `Sound::load/play`, 真正的 OH_AudioRenderer 后续接),
  `lib.rs` 的 alsa 分支排除 ohos:
  ```rust
  #[cfg(any(all(target_os="linux", not(target_env="ohos")), target_os="dragonfly", ...))]
  ```
  并在 Cargo.toml 的 `quad-alsa-sys` 依赖同样排除 ohos

### 坑4: 链接找不到 crtbeginS.o / -lunwind
- **现象**:
  ```
  ld.lld: error: cannot open crtbeginS.o: no such file or directory
  ld.lld: error: unable to find library -lunwind
  ```
- **根因**: SDK 的 clang-runtime 命名是 `clang_rt.crtbegin.o`, 且 libunwind 在
  `llvm/lib/<arch>/libunwind.a`, 不在 clang 默认搜索路径
- **方案**: 把 NDK runtime 复制为 clang 默认查找的名字放进 sysroot:
  ```bash
  # 工具脚本: tools/init_ohos_rt.sh(幂等)
  cp .../llvm/lib/clang/15.0.4/lib/aarch64-linux-ohos/clang_rt.crtbegin.o \
     .../sysroot/usr/lib/aarch64-linux-ohos/crtbeginS.o
  cp .../clang_rt.crtend.o .../crtendS.o
  cp .../llvm/lib/aarch64-linux-ohos/libunwind.a \
     .../sysroot/usr/lib/aarch64-linux-ohos/libunwind.a
  ```

### 坑5: 字体 fontdb 拉 fontconfig(-lX11)
- **现象**: 关掉 miniquad 的 X11 仍报 `-lX11`
- **根因**: fontdb 默认 feature 含 `fontconfig`, 其依赖链链接 X11/pango
- **方案**: 本项目字体全 rust-embed 内嵌, 不查系统字体, 关掉 fontconfig:
  ```toml
  fontdb = { version = "0.16", default-features = false, features = ["fs", "memmap", "std"] }
  ```

### 坑6: ring 等 C 依赖编译
- **现象**: `ring` build 失败 "ToolNotFound: cc"
- **方案**: 设置 `CC_aarch64_unknown_linux_ohos` / `CFLAGS_aarch64_unknown_linux_ohos`
  指向 clang + `--target=aarch64-linux-ohos --sysroot=...`, 见 config 段

## 鸿蒙应用壳(HAR/NAPI)

miniquad 后端编译出 native lib 后, 需要 ArkTS 壳:
1. `XComponent`(承载 EGL surface)上设置 `onSurfaceCreated/onSurfaceChanged/onSurfaceDestroyed`
2. 用 NAPI 暴露给 Rust: 把 surface 传给 `ohos_surface_created`, 尺寸/触摸/按键事件
   转发到 `ohos_surface_changed`/`ohos_touch` 等
3. 参考 [RustDesk → HarmonyOS 移植](https://blog.csdn.net/COLLINSXU/article/details/161957738)
   和 [ohos-rs(napi-rs fork)](https://github.com/tdcare/ohos-rs)

## 参考链接
- [Rust 官方 OpenHarmony 支持](https://android.googlesource.com/toolchain/rustc/+/refs/heads/master/src/doc/rustc/src/platform-support/openharmony.md)
- [Rust 登陆鸿蒙 Native 模块开发](https://rustcc.cn/article?id=568d35d6-b782-49e9-b9b1-5d870d28f927)
- [鸿蒙 PC Rust 库移植全景指南](https://blog.csdn.net/xxx)
