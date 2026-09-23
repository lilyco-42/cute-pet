//! 素材解耦断言(把 THIRD-PARTY-NOTICES §2.2 合规结论固化成测试)。

// ---------------- 素材解耦测试(把 §2.2 合规结论固化成断言) ----------------

#[cfg(test)]
mod asset_decoupling_tests {
    use crate::assets::registry::{CoreAsset, is_character_asset, load_asset};

    /// 铁律: 核心资产表绝不能含任何第三方版权角色素材。
    /// 商业构建(`--no-default-features`)只用这张表 —— 这条挂了就等于商业包漏带素材。
    #[test]
    fn core_asset_excludes_character_assets() {
        let leaked: Vec<String> = CoreAsset::iter()
            .map(|f| f.as_ref().to_string())
            .filter(|p| is_character_asset(p))
            .collect();
        assert!(leaked.is_empty(), "核心资产表混入了角色素材: {leaked:?}");
    }

    /// 字体等原创 / 合规资产必须始终编入 —— 商业版也要能正常跑 UI。
    #[test]
    fn core_asset_contains_font() {
        assert!(CoreAsset::get("font_wenkai.ttf").is_some(), "字体应始终编入");
        assert!(CoreAsset::get("font_wenkai-OFL.txt").is_some(), "OFL 文本应随字体分发");
    }

    /// 测试 / 开发构建(默认 bundle-murasame)下角色素材照旧可用, 现有体验不变。
    #[cfg(feature = "bundle-murasame")]
    #[test]
    fn char_asset_bundled_in_dev_build() {
        assert!(CharAsset::get("murasame_manifest.json").is_some());
        let n = CharAsset::iter().count();
        assert!(n > 100, "角色素材应含 117 张立绘分层图, 实际 {n}");
    }

    /// `PET_ASSETS_DIR` 应被优先查找 —— 这是跨平台 / 真机的通用逃生口。
    /// 用唯一文件名, 避免与其它并行测试的素材查找互相干扰。
    #[test]
    fn pet_assets_dir_env_is_honored() {
        let probe = "cp_assets_probe_unique.bin";
        let tmp = std::env::temp_dir().join(format!("cp_assets_probe_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        std::fs::write(tmp.join(probe), b"ok").unwrap();

        std::env::set_var("PET_ASSETS_DIR", &tmp);
        let got = load_asset(probe);
        std::env::remove_var("PET_ASSETS_DIR");

        let _ = std::fs::remove_file(tmp.join(probe));
        let _ = std::fs::remove_dir_all(&tmp);
        assert!(got.is_ok(), "PET_ASSETS_DIR 指定的目录应被优先查找");
    }

    /// 商业构建下角色素材不可从嵌入表取得(只能走用户素材目录)。
    #[cfg(not(feature = "bundle-murasame"))]
    #[test]
    fn char_asset_absent_in_commercial_build() {
        assert!(CoreAsset::get("murasame_manifest.json").is_none());
        assert!(CoreAsset::get("murasame_corpus.jsonl").is_none());
        // 缺失时应给出友好提示, 而不是 panic
        let err = load_asset("murasame_manifest.json").unwrap_err().to_string();
        assert!(err.contains("角色素材缺失"), "错误信息应提示放置素材, 实际: {err}");
    }
}
