//! 待机动画: 眨眼/说话口型的孪生表情映射。

// ---------------- 眨眼/口型动画 ----------------

/// 孪生表情映射: (基础表情, 眨眼闭眼版, 说话开口版)。
/// 由 `assets/pet/murasame` 的 b/e/m 合成层像素分析生成(闭眼层 = 眼白≈0)。
/// 来源见仓库工具: 对每个基础脸选「嘴部差异最小 + 眼部变化最大」的合成脸作眨眼,
/// 「眼部差异最小 + 嘴部变化最大」的作说话口型。
pub(crate) const FACE_TWINS: &[(&str, &str, &str)] = &[
    ("01", "30", "39"),
    ("02", "33", "39"),
    ("03", "30", "39"),
    ("04", "33", "39"),
    ("05", "33", "34"),
    ("06", "30", "38"),
    ("07", "40", "36"),
    ("08", "40", "36"),
    ("09", "40", "36"),
    ("10", "30", "35"),
    ("11", "30", "39"),
    ("12", "30", "32"),
    ("13", "40", "39"),
    ("14", "30", "39"),
    ("15", "40", "31"),
    ("16", "40", "31"),
    ("17", "37", "36"),
    ("18", "30", "39"),
    ("19", "40", "39"),
    ("20", "33", "39"),
    ("21", "29", "39"),
    ("22", "30", "39"),
    ("23", "28", "34"),
    ("24", "30", "39"),
    ("25", "40", "34"),
    ("26", "40", "35"),
];

/// 当前表情的动画孪生(眨眼/说话)。查不到则返回 None(该表情无动画素材)。
pub(crate) fn face_twins(face: &str) -> Option<(&str, &str)> {
    FACE_TWINS.iter().find(|(b, _, _)| *b == face).map(|(_, bl, tk)| (*bl, *tk))
}

// ---------------- 测试 ----------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 孪生表是**手写数字对**(由合成层的像素差异分析产出), 最容易出的错是复制粘贴
    /// 造成的重复基础脸 —— 那样 `find` 只命中第一条, 后面的条目形同虚设。
    #[test]
    fn twins_table_has_no_duplicate_base_faces() {
        let mut bases: Vec<&str> = FACE_TWINS.iter().map(|(b, _, _)| *b).collect();
        let total = bases.len();
        bases.sort_unstable();
        bases.dedup();
        assert_eq!(total, bases.len(), "FACE_TWINS 有重复的基础表情编号(后面的条目永远不会生效)");
    }

    /// 每条都必须能查到且三个字段非空: 空白编号会"命中却拿不到贴图"。
    #[test]
    fn every_twin_entry_is_resolvable() {
        for (base, blink, talk) in FACE_TWINS {
            assert!(!base.is_empty() && !blink.is_empty() && !talk.is_empty(), "有空白编号");
            assert_eq!(
                face_twins(base),
                Some((*blink, *talk)),
                "基础表情 {base} 查不到自己的孪生脸"
            );
        }
    }

    /// 未登记的表情必须返回 None, 让调用方走"无动画"降级而不是 panic。
    #[test]
    fn unknown_face_yields_none() {
        assert_eq!(face_twins(""), None);
        assert_eq!(face_twins("999"), None);
        assert_eq!(face_twins("smile"), None); // 只认表里的两位数字编号
    }
}
