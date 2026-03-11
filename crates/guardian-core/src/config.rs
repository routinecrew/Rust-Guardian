//! Config 로딩 + 핫 리로드

use crate::contracts::GuardianConfig;
use anyhow::Result;
use std::path::Path;

/// YAML 설정 파일을 로딩한다.
pub fn load_config(path: &Path) -> Result<GuardianConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: GuardianConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}
