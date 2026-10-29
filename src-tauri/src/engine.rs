//! 定位 PrismaRev 源码根（引擎 + 编辑器源码）。
//!
//! 顺序（先命中先用）：
//!
//! 1. `PRISMREV_ROOT` 环境变量——显式覆盖；设置了却无效属于错误，不回退，
//!    否则会静默用别的 checkout，比直接报错更难查。
//! 2. 设置页保存的 `engineRoot`。
//! 3. Hub 可执行文件的各级父目录（开发布局：`<repo>/hub/src-tauri/target/debug`）。
//! 4. Hub 可执行文件各级父目录的同级目录（常见布局：`~/src/PrismaRev` + `~/src/prisma-hub`）。
//!
//! 「是引擎根」的判定同时要求 `crates/prism-app`、`crates/prism-engine` 和
//! `editor/src-tauri/tauri.conf.json` 存在——只看一个目录名不够可靠。

use serde::Serialize;
use std::path::{Path, PathBuf};

pub const ENGINE_ROOT_ENV: &str = "PRISMREV_ROOT";

/// 引擎根的来源，前端据此显示「来自哪里」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineSource {
    Env,
    Setting,
    Ancestor,
    Sibling,
    None,
}

impl EngineSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Env => "env",
            Self::Setting => "setting",
            Self::Ancestor => "ancestor",
            Self::Sibling => "sibling",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    /// 命中的源码根；`None` 表示没找到。
    pub root: Option<String>,
    pub source: &'static str,
    /// 设置页保存的值（可能和 `root` 不同：保存的路径已失效时仍回显）。
    pub setting: Option<String>,
    /// `editor/src-tauri/tauri.conf.json` 里的版本，用于给编辑器版本兜底。
    pub editor_version: Option<String>,
    /// 环境变量 / 设置值无效时的说明。
    pub hint: Option<String>,
}

impl EngineInfo {
    pub fn root_path(&self) -> Option<PathBuf> {
        self.root.as_ref().map(PathBuf::from)
    }
}

pub fn is_engine_root(path: &Path) -> bool {
    path.join("crates/prism-app/Cargo.toml").is_file()
        && path.join("crates/prism-engine/Cargo.toml").is_file()
        && path.join("editor/src-tauri/tauri.conf.json").is_file()
}

/// 读取引擎 checkout 里编辑器的版本号（`editor/src-tauri/tauri.conf.json`）。
pub fn editor_version(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("editor/src-tauri/tauri.conf.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("version")
        .and_then(|version| version.as_str())
        .map(str::to_string)
}

/// 从 `start` 逐级向上找引擎根。
pub fn ancestor_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| is_engine_root(dir))
        .map(Path::to_path_buf)
}

/// Hub 自己的 checkout 根：同时有 `src-tauri/Cargo.toml` 和 `package.json`。
fn hub_checkout_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| {
            dir.join("src-tauri/Cargo.toml").is_file() && dir.join("package.json").is_file()
        })
        .map(Path::to_path_buf)
}

/// 在 Hub checkout 的父目录里找「同级」的引擎 checkout。
///
/// 只在已知的 checkout 父目录里扫一层，避免在 `/tmp` 或 `$HOME` 这种位置扫到
/// 无关的仓库。打包安装（没有 checkout）不做同级探测。
pub fn sibling_root(start: &Path) -> Option<PathBuf> {
    let parent = hub_checkout_root(start)?.parent()?.to_path_buf();
    let Ok(entries) = std::fs::read_dir(&parent) else {
        return None;
    };
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.path())
        .filter(|path| {
            !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
        })
        .collect();
    candidates.sort();
    candidates.into_iter().find(|path| is_engine_root(path))
}

/// 解析引擎根。`setting` 是设置页保存的路径，`env` 是环境变量值。
pub fn detect(exe_dir: &Path, setting: Option<&str>, env: Option<&str>) -> EngineInfo {
    let setting_owned = setting
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if let Some(raw) = env.map(str::trim).filter(|value| !value.is_empty()) {
        let path = PathBuf::from(raw);
        if is_engine_root(&path) {
            let root = crate::util::canonical_string(&path);
            return EngineInfo {
                editor_version: editor_version(&path),
                root: Some(root),
                source: EngineSource::Env.label(),
                setting: setting_owned,
                hint: None,
            };
        }
        return EngineInfo {
            root: None,
            source: EngineSource::Env.label(),
            setting: setting_owned,
            editor_version: None,
            hint: Some(format!(
                "{ENGINE_ROOT_ENV} 指向的目录不是 PrismaRev 源码根（缺少 crates/prism-app 或 editor/src-tauri）：{}",
                path.display()
            )),
        };
    }

    // 设置值无效时继续自动探测，但把「为什么没用设置」带回 UI。
    let mut invalid_setting: Option<String> = None;
    if let Some(raw) = setting_owned.as_deref() {
        let path = PathBuf::from(raw);
        if is_engine_root(&path) {
            return EngineInfo {
                editor_version: editor_version(&path),
                root: Some(crate::util::canonical_string(&path)),
                source: EngineSource::Setting.label(),
                setting: setting_owned,
                hint: None,
            };
        }
        invalid_setting = Some(raw.to_string());
    }

    let setting_hint = || {
        invalid_setting
            .as_deref()
            .map(|raw| format!("设置页保存的源码根无效，已改用自动探测：{raw}"))
    };

    for (source, found) in [
        (EngineSource::Ancestor, ancestor_root(exe_dir)),
        (EngineSource::Sibling, sibling_root(exe_dir)),
    ] {
        if let Some(path) = found {
            return EngineInfo {
                editor_version: editor_version(&path),
                root: Some(crate::util::canonical_string(&path)),
                source: source.label(),
                setting: setting_owned,
                hint: setting_hint(),
            };
        }
    }

    EngineInfo {
        root: None,
        source: EngineSource::None.label(),
        setting: setting_owned,
        editor_version: None,
        hint: Some(match invalid_setting {
            Some(raw) => format!(
                "设置页保存的源码根无效：{raw}。也可以设置 {ENGINE_ROOT_ENV}，\
                 或把 Hub 和 PrismaRev 放在同一父目录下。"
            ),
            None => format!(
                "没有找到 PrismaRev 源码。设置 {ENGINE_ROOT_ENV}、在设置页选择源码目录，\
                 或把 Hub 和 PrismaRev 放在同一父目录下。"
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("prisma-hub-engine-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("创建父目录");
        }
        std::fs::write(path, b"fake").expect("写文件");
    }

    fn fake_engine(root: &Path, version: &str) {
        touch(&root.join("crates/prism-app/Cargo.toml"));
        touch(&root.join("crates/prism-engine/Cargo.toml"));
        touch(&root.join("editor/src-tauri/tauri.conf.json"));
        std::fs::write(
            root.join("editor/src-tauri/tauri.conf.json"),
            format!(r#"{{"version":"{version}"}}"#),
        )
        .expect("写 tauri.conf.json");
    }

    #[test]
    fn a_checkout_needs_crates_and_the_editor() {
        let root = scratch("shape");
        assert!(!is_engine_root(&root));
        touch(&root.join("crates/prism-app/Cargo.toml"));
        assert!(!is_engine_root(&root));
        touch(&root.join("crates/prism-engine/Cargo.toml"));
        assert!(!is_engine_root(&root));
        touch(&root.join("editor/src-tauri/tauri.conf.json"));
        assert!(is_engine_root(&root));
    }

    #[test]
    fn env_wins_and_must_be_valid() {
        let root = scratch("env");
        let engine = root.join("engine");
        fake_engine(&engine, "1.2.3");
        let exe_dir = root.join("hub/src-tauri/target/debug");
        std::fs::create_dir_all(&exe_dir).expect("创建 exe 目录");

        let info = detect(&exe_dir, None, Some(&engine.to_string_lossy()));
        assert_eq!(info.source, "env");
        assert_eq!(info.editor_version.as_deref(), Some("1.2.3"));

        let missing = root.join("nope");
        let info = detect(&exe_dir, None, Some(&missing.to_string_lossy()));
        assert!(info.root.is_none());
        assert!(info.hint.is_some_and(|hint| hint.contains(ENGINE_ROOT_ENV)));
    }

    #[test]
    fn saved_setting_beats_auto_detection() {
        let root = scratch("setting");
        let engine = root.join("engine");
        fake_engine(&engine, "2.0.0");
        let exe_dir = engine.join("hub/src-tauri/target/debug");
        std::fs::create_dir_all(&exe_dir).expect("创建 exe 目录");

        // 自动探测也能命中（exe 在 engine 里），但保存的设置应优先。
        let other = root.join("other");
        fake_engine(&other, "3.0.0");
        let info = detect(&exe_dir, Some(&other.to_string_lossy()), None);
        assert_eq!(info.source, "setting");
        assert_eq!(
            info.root.as_deref(),
            Some(other.canonicalize().unwrap().to_string_lossy().as_ref())
        );
    }

    #[test]
    fn invalid_setting_falls_back_with_a_hint() {
        let root = scratch("bad-setting");
        fake_engine(&root, "0.1.0");
        let exe_dir = root.join("hub/src-tauri/target/debug");
        std::fs::create_dir_all(&exe_dir).expect("创建 exe 目录");

        let info = detect(&exe_dir, Some("/definitely/not/a/checkout"), None);
        assert_eq!(info.source, "ancestor");
        assert!(info.root.is_some());
        assert!(info
            .hint
            .is_some_and(|hint| hint.contains("设置页保存的源码根无效")));
    }

    #[test]
    fn ancestor_layout_is_found() {
        let root = scratch("ancestor");
        fake_engine(&root, "0.1.0");
        let exe_dir = root.join("hub/src-tauri/target/debug");
        std::fs::create_dir_all(&exe_dir).expect("创建 exe 目录");

        let info = detect(&exe_dir, None, None);
        assert_eq!(info.source, "ancestor");
        assert!(info.root.is_some());
    }

    #[test]
    fn sibling_layout_is_found() {
        let root = scratch("sibling");
        let engine = root.join("PrismaRev");
        fake_engine(&engine, "0.1.0");
        let hub = root.join("prisma-hub");
        touch(&hub.join("package.json"));
        touch(&hub.join("src-tauri/Cargo.toml"));
        let exe_dir = hub.join("src-tauri/target/debug");
        std::fs::create_dir_all(&exe_dir).expect("创建 exe 目录");

        let info = detect(&exe_dir, None, None);
        assert_eq!(info.source, "sibling");
        assert!(info.root.is_some());
    }

    #[test]
    fn nothing_found_reports_a_hint() {
        let root = scratch("none");
        let exe_dir = root.join("hub/src-tauri/target/debug");
        std::fs::create_dir_all(&exe_dir).expect("创建 exe 目录");
        let info = detect(&exe_dir, None, None);
        assert_eq!(info.source, "none");
        assert!(info.root.is_none());
        assert!(info.hint.is_some());
    }
}
