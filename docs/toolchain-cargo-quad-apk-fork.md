# 工具链结论: manifest 声明 Android Service(cargo-quad-apk-ply)

> 结论: **无需补丁** —— plyx 实际使用的 `cargo-quad-apk-ply`
> (TheRedDeveloper 维护, cargo lib 0.87) **原生支持**在 Cargo.toml 里声明
> Android Service(含 `foregroundServiceType`)。

## 探索过程(为何一度以为要打补丁)

1. 安装版 `cargo-quad-apk`(plyx 构建时自装到 `persist\rustup\.cargo\bin\`)
   是 macnelly 旧系, 只支持 `quad.toml` 的 `java_files`, 无 manifest service 支持。
2. 我基于 macnelly fork 写了 `java_service_attrs` 补丁
   (仓库 [lilyco-42/cargo-quad-apk](https://github.com/lilyco-42/cargo-quad-apk),
   详见 git log `c7740ff`), 但 macnelly fork 用 `cargo = "0.62"` 旧库,
   读不了 Cargo.lock v4, 且 plyx 实际调用的不是它。
3. 真相: plyx 从 `https://github.com/TheRedDeveloper/cargo-quad-apk-ply` 安装
   它自己的 fork(cargo lib 0.87), 该 fork **原生支持**:
   ```toml
   [[package.metadata.android.service]]
   name = "rust.cute_pet.ScreenCaptureService"
   foreground_service_type = "mediaProjection"
   exported = false
   ```
   (见其 `src/config.rs` `TomlService` + `src/ops/build.rs` 的 `{services}` 渲染)

## 实际落地方案(cute-pet)

- `pet/Cargo.toml [package.metadata.android]`:
  - `[[package.metadata.android.service]]` 声明 ScreenCaptureService(mediaProjection)
  - `android_version = 36 / target_sdk_version = 36`(fork 默认 31, 本机只有 35/36)
  - `android:debuggable = "true"`(开发期, 供 `adb run-as` 注入运行时配置)
- `pet/quad.toml`: 仅 `java_files = ["java/ScreenCaptureService.java"]`
- 构建命令不变: `plyx apk --native --auto`
- 验证: `aapt2 dump xmltree` 见
  `<service android:name="rust.cute_pet.ScreenCaptureService"
   android:foregroundServiceType="mediaProjection">`

## 工具链二进制位置(scoop rustup)

- `apps\rustup\current` → `apps\rustup\1.29.0`(软链), 其 `.cargo` → `persist\rustup\.cargo`
- 构建实际解析 `persist\rustup\.cargo\bin\cargo-quad-apk.exe`
- 若需自编译该工具: `cargo install --path tools/cargo-quad-apk-ply --force`
  (cargo lib 0.87, 编译约 10 分钟)
