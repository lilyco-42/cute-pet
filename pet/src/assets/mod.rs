//! 资产域: 嵌入资产注册表(registry) + 立绘 manifest 模型(manifest) + 合规断言(compliance)。

pub(crate) mod manifest;
pub(crate) mod registry;
pub(crate) use manifest::*;
pub(crate) use registry::*;

#[cfg(test)]
mod compliance;
