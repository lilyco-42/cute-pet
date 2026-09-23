//! 资产注册表: 嵌入资产(CharAsset/CoreAsset) + 运行时资产(网络导入) + 外部素材目录。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};


pub(crate) const MANIFEST_PATH: &str = "murasame_manifest.json";
pub(crate) const LAYER_DIR: &str = "murasame_layers";
// 内置原创默认吉祥物(随所有构建分发, 合规, 非第三方版权): 商业包缺丛雨素材时回退显示。
pub(crate) const DEFAULT_PET_MANIFEST: &str = "default_pet_manifest.json";
pub(crate) const DEFAULT_PET_LAYER_DIR: &str = "default_pet_layers";
// 缩放: 桌面 1/3(600x850 窗口), 移动端/鸿蒙放大填满屏幕
#[cfg(any(target_os = "android", target_env = "ohos"))]
pub(crate) const SCALE: f32 = 0.6;
#[cfg(not(any(target_os = "android", target_env = "ohos")))]
pub(crate) const SCALE: f32 = 1.0 / 3.0;

/// 跨平台资产: 编译期嵌入二进制(rust-embed), 所有平台统一, 无运行时路径问题。
/// 第三方版权角色素材(丛雨/むらさめ)的路径前缀。
/// 版权归柚子社《千恋＊万花》、不归我方 —— 商业构建(`--no-default-features`,
/// 无 bundle-murasame)不编入二进制, 运行时改从用户素材目录加载。
pub(crate) const CHARACTER_ASSET_PREFIXES: [&str; 4] = [
    "murasame_manifest.json",
    "murasame_persona.txt",
    "murasame_corpus",
    "murasame_layers/",
];

/// 是否属于第三方版权角色素材(而非我方原创资产)。
pub(crate) fn is_character_asset(p: &str) -> bool {
    CHARACTER_ASSET_PREFIXES.iter().any(|s| p.starts_with(s))
}

/// 只含角色素材的嵌入表 —— 仅在 bundle-murasame(默认: 开发 / 测试构建)下编译。
#[cfg(feature = "bundle-murasame")]
#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
#[include = "murasame_*"]
pub(crate) struct CharAsset;

/// 核心资产(字体 / UI 预设 / 对话库等): 始终编入, 且已排除角色素材。
/// 商业构建靠这张表就够 —— 二进制里不含任何柚子社素材。
#[derive(rust_embed::RustEmbed)]
#[folder = "assets/"]
#[exclude = "murasame_*"]
pub(crate) struct CoreAsset;

/// 用户素材目录: 角色素材不随商业二进制分发, 运行时**优先**从这里读。
/// 桌面端 = 可执行文件同级的 assets/ 目录(把素材放程序旁边即可)。
/// 用户素材目录候选(按优先级)。角色素材不随商业二进制分发, 运行时**优先**从这里读。
///
/// 1. `PET_ASSETS_DIR` 环境变量 —— 跨平台通用逃生口, 真机路径不确定时用它直接指定。
/// 2. 可执行文件同级 `assets/`(桌面)。
/// 3. 平台专属目录:
///    - Android: app 专属外部目录(无需权限, 本项目 minSdk 26 满足) + `/sdcard` 兜底
///    - HarmonyOS: el2 应用沙箱 files 目录
///    - iOS: App Documents(可通过「文件」App 放入)
///    - WASM: 浏览器没有本地文件系统, **不支持**外部目录
pub(crate) fn external_asset_dirs() -> Vec<std::path::PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();

        if let Some(d) = std::env::var_os("PET_ASSETS_DIR") {
            dirs.push(std::path::PathBuf::from(d));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                dirs.push(parent.join("assets"));
            }
        }

        #[cfg(target_os = "android")]
        {
            // app 专属外部目录: 无需任何权限即可读写(API 19+)
            dirs.push(std::path::PathBuf::from(
                "/sdcard/Android/data/rust.cute_pet/files/assets",
            ));
            // 兜底: 用户用文件管理器直接放 sdcard 根目录
            dirs.push(std::path::PathBuf::from("/sdcard/cute-pet/assets"));
        }

        #[cfg(target_env = "ohos")]
        {
            dirs.push(std::path::PathBuf::from(
                "/data/storage/el2/base/haps/entry/files/assets",
            ));
        }

        #[cfg(target_os = "ios")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                dirs.push(std::path::PathBuf::from(home).join("Documents").join("assets"));
            }
        }

        dirs
    }
}

pub(crate) fn character_asset_missing_msg(p: &str) -> String {
    let dirs = external_asset_dirs();
    if dirs.is_empty() {
        // WASM: 浏览器没有本地文件系统, 用户无法放置素材
        format!(
            "角色素材缺失: {p}\n本版本不含第三方版权角色素材(版权归柚子社《千恋＊万花》)。\n\
             当前平台(WASM)无法读取本地素材目录 —— 需要角色请使用桌面版, 并把素材放到程序同级的 assets/ 目录。"
        )
    } else {
        let list = dirs
            .iter()
            .map(|d| format!("  - {}", d.display()))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "角色素材缺失: {p}\n本版本不含第三方版权角色素材(版权归柚子社《千恋＊万花》)。\n\
             请把角色素材放到以下任一目录(也可用 PET_ASSETS_DIR 环境变量指定):\n{list}"
        )
    }
}

/// 运行时注入的资产(wasm / webview 壳「网络导入」用)。
/// 原生构建走 external_asset_dirs 读本地文件; 只有 wasm 构建无本地 FS,
/// load_asset 只读编译期嵌入(rust-embed)。为支持「网络导入」, JS 侧 fetch 素材后
/// 经 cute_pet_register_asset 写入本表, load_asset 优先返回 —— 这样不重新编译 wasm
/// 也能在运行时加载第三方角色素材。低频一次性操作, 用 OnceLock + Mutex 足够。
pub(crate) static RUNTIME_ASSETS: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
pub(crate) fn runtime_assets() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    RUNTIME_ASSETS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn load_asset(p: &str) -> anyhow::Result<Vec<u8>> {
    // 0) 运行时注入(网络导入)优先 —— 让 JS 桥能在不重编 wasm 的情况下塞入素材
    if let Some(b) = runtime_assets().lock().unwrap().get(p) {
        return Ok(b.clone());
    }
    // 1) 用户素材目录优先 —— 商业版靠这一条拿到素材(按优先级逐个尝试)
    for dir in external_asset_dirs() {
        if let Ok(b) = std::fs::read(dir.join(p)) {
            return Ok(b);
        }
    }
    // 2) 编译期嵌入
    if is_character_asset(p) {
        #[cfg(feature = "bundle-murasame")]
        if let Some(f) = CharAsset::get(p) {
            return Ok(f.data.into_owned());
        }
        anyhow::bail!("{}", character_asset_missing_msg(p))
    } else if let Some(f) = CoreAsset::get(p) {
        Ok(f.data.into_owned())
    } else {
        anyhow::bail!("资产缺失: {p}")
    }
}

/// 角色素材加载失败时打印提示并降级(不 panic)。
/// 商业版正常情况下就会走这条路径 —— 缺素材属于预期, 不是崩溃理由。
pub(crate) fn load_character_asset_or_warn(p: &str) -> Option<Vec<u8>> {
    match load_asset(p) {
        Ok(b) => Some(b),
        Err(e) => {
            eprintln!("[cute-pet] {e}");
            None
        }
    }
}

/// wasm「网络导入」桥: 在 wasm 线性内存分配 len 字节并返回指针, 由 JS 写入素材字节后
/// 调用 cute_pet_register_asset。用 Vec::forget 持有内存(低频一次性, 泄漏可接受)。
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn cute_pet_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::with_capacity(len);
    let ptr = v.as_mut_ptr();
    std::mem::forget(v);
    ptr
}

/// wasm「网络导入」桥: 注册一个运行时资产(路径 + 字节), 之后 load_asset 优先返回它。
/// 由 JS 侧 fetch 素材后调用, 使引擎无需重编即可在运行时加载第三方角色素材。
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn cute_pet_register_asset(
    p_ptr: *const u8,
    p_len: usize,
    d_ptr: *const u8,
    d_len: usize,
) {
    let path = match std::str::from_utf8(unsafe { std::slice::from_raw_parts(p_ptr, p_len) }) {
        Ok(s) => s.to_string(),
        Err(_) => return,
    };
    let data = unsafe { std::slice::from_raw_parts(d_ptr, d_len) }.to_vec();
    runtime_assets().lock().unwrap().insert(path, data);
}
