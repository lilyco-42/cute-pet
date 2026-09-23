//! manifest 模型: 立绘分层的资产描述(人物/姿势/表情/图层)。

use std::collections::HashMap;

use serde::Deserialize;

// ---------------- manifest 模型 ----------------

#[derive(Deserialize, Clone)]
pub(crate) struct Manifest {
    pub(crate) character: String,
    pub(crate) name_cn: String,
    pub(crate) voice_code: String,
    pub(crate) sets: HashMap<String, SetManifest>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct SetManifest {
    #[allow(dead_code)]
    pub(crate) stand: Stand,
    pub(crate) info: Info,
    pub(crate) composition: Composition,
    #[allow(dead_code)]
    pub(crate) layer_count: usize,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Stand {
    #[allow(dead_code)]
    pub(crate) filename: String,
    #[allow(dead_code)]
    pub(crate) xoffset: i32,
    #[allow(dead_code)]
    pub(crate) yoffset: i32,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Info {
    pub(crate) dress: Vec<DressRow>,
    pub(crate) face: Vec<FaceRow>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct DressRow {
    pub(crate) dress: String,
    pub(crate) diff: u32,
    pub(crate) layer: String,
}

#[derive(Deserialize, Clone)]
pub(crate) struct FaceRow {
    pub(crate) face: String,
    pub(crate) layer: String,
}

#[derive(Deserialize, Clone)]
pub(crate) struct Composition {
    pub(crate) groups: HashMap<String, u32>,
    pub(crate) items: HashMap<String, LayerItem>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct LayerItem {
    #[allow(dead_code)]
    pub(crate) layer_type: u32,
    pub(crate) name: String,
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) w: u32,
    pub(crate) h: u32,
    pub(crate) opacity: u32,
    #[allow(dead_code)]
    pub(crate) visible: u32,
    pub(crate) layer_id: u32,
    pub(crate) group: u32,
}

