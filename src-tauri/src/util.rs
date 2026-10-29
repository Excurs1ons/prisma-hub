//! 路径 / 时间等小工具。

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn current_exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_default()
}

pub fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

/// Windows 路径大小写不敏感，其它平台按字节比较。
pub fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// 把用户输入解析成真实存在的目录。
pub fn canonical_dir(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("请选择目录".to_string());
    }
    let path = PathBuf::from(trimmed);
    let path = path
        .canonicalize()
        .map_err(|error| format!("无法访问目录 {}: {error}", path.display()))?;
    if !path.is_dir() {
        return Err(format!("不是目录：{}", path.display()));
    }
    Ok(path)
}

/// 尽量 canonicalize；路径已不存在时保留用户输入（用于「移除已丢失的工程」）。
pub fn canonical_or_raw(raw: &str) -> String {
    PathBuf::from(raw.trim())
        .canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| raw.trim().to_string())
}

/// 字符串路径 → canonical 路径；失败时原样返回。
pub fn canonical_string(path: &Path) -> String {
    path.canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

pub fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("创建目录 {} 失败: {error}", parent.display()))?;
    }
    std::fs::write(path, contents).map_err(|error| format!("写入 {} 失败: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dir_input_is_rejected() {
        assert!(canonical_dir("   ").is_err());
    }

    #[test]
    fn same_path_is_platform_aware() {
        assert!(same_path("a/b", "a/b"));
        assert_eq!(same_path("A/B", "a/b"), cfg!(windows));
    }
}
