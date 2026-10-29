//! `hub.json` 的读写。
//!
//! 位置：`app_data_dir()/hub.json`。只存「用户的选择」——工程路径、手动添加的
//! 编辑器、默认编辑器、引擎源码根。工程和编辑器的当前状态（是否存在、版本号、
//! 场景数）每次从磁盘重新探测，避免 Hub 拿着过期数据骗人。
//!
//! v1 只有 `projects`；v2 增加了 `engineRoot` / `editors` / `defaultEditor`。
//! 旧文件靠 `#[serde(default)]` 直接升级，不需要单独的迁移步骤。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const STORE_VERSION: u32 = 2;
pub const STORE_FILE: &str = "hub.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredProject {
    pub path: String,
    #[serde(default)]
    pub added_at: u64,
    #[serde(default)]
    pub last_opened: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEditor {
    pub path: String,
    #[serde(default)]
    pub added_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubStore {
    #[serde(default = "store_version")]
    pub version: u32,
    /// 用户手动指定的 PrismaRev 源码根（设置页）。
    #[serde(default)]
    pub engine_root: Option<String>,
    #[serde(default)]
    pub projects: Vec<StoredProject>,
    /// 手动添加的编辑器安装。
    #[serde(default)]
    pub editors: Vec<StoredEditor>,
    /// 默认编辑器的二进制路径。
    #[serde(default)]
    pub default_editor: Option<String>,
}

fn store_version() -> u32 {
    STORE_VERSION
}

impl Default for HubStore {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            engine_root: None,
            projects: Vec::new(),
            editors: Vec::new(),
            default_editor: None,
        }
    }
}

impl HubStore {
    /// 读取 `hub.json`；不存在或损坏时返回默认值（损坏时只 warn，不阻塞启动）。
    pub fn load(path: &Path) -> (Self, bool) {
        let first_run = !path.is_file();
        if first_run {
            return (Self::default(), true);
        }
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Self>(&text) {
                Ok(mut store) => {
                    store.version = STORE_VERSION;
                    (store, false)
                }
                Err(error) => {
                    log::warn!("{} 解析失败，使用空配置: {error}", path.display());
                    (Self::default(), false)
                }
            },
            Err(error) => {
                log::warn!("读取 {} 失败: {error}", path.display());
                (Self::default(), false)
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| format!("序列化 hub.json 失败: {error}"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("创建应用数据目录失败: {error}"))?;
        }
        std::fs::write(path, text)
            .map_err(|error| format!("写入 {} 失败: {error}", path.display()))
    }
}

/// 应用数据目录下的 `hub.json` 路径。
pub fn store_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(STORE_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "prisma-hub-store-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    #[test]
    fn v1_store_upgrades_to_v2_defaults() {
        let dir = scratch("v1");
        let path = store_path(&dir);
        std::fs::write(
            &path,
            r#"{"version":1,"projects":[{"path":"/tmp/demo","addedAt":1,"lastOpened":null}]}"#,
        )
        .expect("写旧版本");

        let (store, first_run) = HubStore::load(&path);
        assert!(!first_run);
        assert_eq!(store.version, STORE_VERSION);
        assert_eq!(store.projects.len(), 1);
        assert!(store.engine_root.is_none());
        assert!(store.editors.is_empty());
        assert!(store.default_editor.is_none());
    }

    #[test]
    fn missing_store_is_first_run() {
        let dir = scratch("missing");
        let (store, first_run) = HubStore::load(&store_path(&dir));
        assert!(first_run);
        assert_eq!(store.version, STORE_VERSION);
    }

    #[test]
    fn corrupt_store_falls_back_to_defaults() {
        let dir = scratch("corrupt");
        let path = store_path(&dir);
        std::fs::write(&path, "{ not json").expect("写坏文件");
        let (store, first_run) = HubStore::load(&path);
        assert!(!first_run);
        assert!(store.projects.is_empty());
    }

    #[test]
    fn round_trip_keeps_selections() {
        let dir = scratch("round-trip");
        let path = store_path(&dir);
        let store = HubStore {
            version: STORE_VERSION,
            engine_root: Some("/engine".to_string()),
            projects: vec![StoredProject {
                path: "/demo".to_string(),
                added_at: 7,
                last_opened: Some(9),
            }],
            editors: vec![StoredEditor {
                path: "/bin/prisma-editor".to_string(),
                added_at: 3,
            }],
            default_editor: Some("/bin/prisma-editor".to_string()),
        };
        store.save(&path).expect("保存");

        let (loaded, first_run) = HubStore::load(&path);
        assert!(!first_run);
        assert_eq!(loaded.engine_root.as_deref(), Some("/engine"));
        assert_eq!(loaded.projects[0].last_opened, Some(9));
        assert_eq!(loaded.editors[0].path, "/bin/prisma-editor");
        assert_eq!(loaded.default_editor.as_deref(), Some("/bin/prisma-editor"));
    }
}
