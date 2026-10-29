//! 工程识别：结构探测、版本标记、新建骨架。
//!
//! ## 版本标记
//!
//! 和 Unity 一样用 `ProjectSettings/ProjectVersion.txt`：
//!
//! ```text
//! m_EditorVersion: 0.1.0
//! ```
//!
//! 它只回答「这个工程上次用哪个编辑器打开」。Hub 打开工程时：标记里的版本有对应
//! 安装就用它，否则用默认编辑器，并在启动后把标记更新成实际使用的版本。
//!
//! ## 结构探测
//!
//! 不要求工程「完整」才算存在——`assets/scenes` 决定能不能打开，其余目录只用于在
//! 详情页说明缺什么（例如没有游戏 crate 的纯资源工程是合法的）。

use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::util::write_file;

/// 版本标记相对工程根的位置。
pub const VERSION_FILE: &str = "ProjectSettings/ProjectVersion.txt";
/// 版本标记里唯一的键（Unity 兼容）。
pub const VERSION_KEY: &str = "m_EditorVersion";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStructure {
    pub assets: bool,
    pub scenes_dir: bool,
    /// `assets/scenes.toml`（运行时场景清单）。
    pub scenes_manifest: bool,
    /// `assets/settings.toml`（`[app]` / `[window]` / `[render]` / `[build]`）。
    pub settings: bool,
    /// 根目录 `Cargo.toml`（游戏 crate）。
    pub game_crate: bool,
    /// `ProjectSettings/`。
    pub project_settings: bool,
    /// `ProjectSettings/ProjectVersion.txt`。
    pub version_file: bool,
}

impl ProjectStructure {
    /// 目录不存在时的空结构，避免为不存在的路径调用 `structure_of`。
    fn missing() -> Self {
        Self {
            assets: false,
            scenes_dir: false,
            scenes_manifest: false,
            settings: false,
            game_crate: false,
            project_settings: false,
            version_file: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    pub path: String,
    pub name: String,
    pub exists: bool,
    /// 能否作为工程打开（存在且 `assets/scenes` 是目录）。
    pub valid: bool,
    pub scene_count: usize,
    pub has_game_crate: bool,
    /// 版本标记里的编辑器版本。
    pub editor_version: Option<String>,
    /// 版本标记文件是否存在。
    pub has_version_file: bool,
    pub structure: ProjectStructure,
    pub last_opened: Option<u64>,
    pub added_at: u64,
    pub running: bool,
}

/// 读取 `ProjectSettings/ProjectVersion.txt` 的 `m_EditorVersion`。
pub fn read_project_version(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(VERSION_FILE)).ok()?;
    parse_version_file(&text)
}

pub fn parse_version_file(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == VERSION_KEY)
            .then(|| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

/// 写（或升级）版本标记。目录不存在时自动创建。
pub fn write_project_version(root: &Path, version: &str) -> Result<(), String> {
    let version = version.trim();
    if version.is_empty() {
        return Err("编辑器版本为空，无法写工程版本标记".to_string());
    }
    write_file(
        &root.join(VERSION_FILE),
        &format!("{VERSION_KEY}: {version}\n"),
    )
}

pub fn is_project_root(path: &Path) -> bool {
    path.join("assets/scenes").is_dir()
}

pub fn structure_of(root: &Path) -> ProjectStructure {
    ProjectStructure {
        assets: root.join("assets").is_dir(),
        scenes_dir: root.join("assets/scenes").is_dir(),
        scenes_manifest: root.join("assets/scenes.toml").is_file(),
        settings: root.join("assets/settings.toml").is_file(),
        game_crate: root.join("Cargo.toml").is_file(),
        project_settings: root.join("ProjectSettings").is_dir(),
        version_file: root.join(VERSION_FILE).is_file(),
    }
}

pub fn count_scenes(root: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(root.join("assets/scenes")) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name.ends_with(".scene.json"))
                .unwrap_or(false)
        })
        .count()
}

/// 工程里的游戏 crate：仓库布局优先 `projects/prisma`，否则任意
/// `projects/*/Cargo.toml`；独立工程则用根目录的 `Cargo.toml`。
pub fn find_game_manifest(root: &Path) -> Option<PathBuf> {
    let preferred = root.join("projects/prisma/Cargo.toml");
    if preferred.is_file() {
        return Some(preferred);
    }
    if let Ok(entries) = std::fs::read_dir(root.join("projects")) {
        for entry in entries.flatten() {
            let manifest = entry.path().join("Cargo.toml");
            if manifest.is_file() {
                return Some(manifest);
            }
        }
    }
    let manifest = root.join("Cargo.toml");
    manifest.is_file().then_some(manifest)
}

pub fn project_name(root: &Path, fallback: &str) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

/// 用磁盘上的当前状态构造条目；`running` 由调用方提供。
pub fn entry(root: &Path, stored_path: &str, added_at: u64, last_opened: Option<u64>, running: bool) -> ProjectEntry {
    let exists = root.is_dir();
    let structure = if exists {
        structure_of(root)
    } else {
        ProjectStructure::missing()
    };
    let editor_version = if exists {
        read_project_version(root)
    } else {
        None
    };
    ProjectEntry {
        path: stored_path.to_string(),
        name: project_name(root, stored_path),
        exists,
        valid: exists && structure.scenes_dir,
        scene_count: if exists { count_scenes(root) } else { 0 },
        has_game_crate: exists && find_game_manifest(root).is_some(),
        has_version_file: structure.version_file,
        editor_version,
        structure,
        last_opened,
        added_at,
        running,
    }
}

// ---------------------------------------------------------------------------
// 命名
// ---------------------------------------------------------------------------

pub fn validate_project_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("工程名称不能为空".to_string());
    }
    if name == "." || name == ".." {
        return Err("工程名称不能是 `.` 或 `..`".to_string());
    }
    if name.chars().any(|ch| {
        ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    }) {
        return Err("工程名称不能包含路径分隔符或特殊字符".to_string());
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err("工程名称不能以 `.` 或空格结尾".to_string());
    }
    Ok(())
}

/// Cargo package 名：只保留 ASCII 字母数字 / `-` / `_`，其余折叠成 `-`。
/// 中文工程名会退化成 `game`，但目录名保持原样。
pub fn crate_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    out = out.trim_matches('-').to_string();
    if out.is_empty() {
        out = "game".to_string();
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        out = format!("game-{out}");
    }
    out
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace('"', "\\\"")
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

// ---------------------------------------------------------------------------
// 新建工程骨架
// ---------------------------------------------------------------------------

/// 生成工程骨架。
///
/// - 资源骨架（场景 / 清单 / 设置 / `.gitignore`）总是生成；
/// - `editor_version` 有值时写 `ProjectSettings/ProjectVersion.txt`；
/// - `engine_root` 有值时才生成游戏 crate（依赖用绝对路径，工程可以放在任意位置）。
pub fn write_project_skeleton(
    target: &Path,
    name: &str,
    engine_root: Option<&Path>,
    editor_version: Option<&str>,
) -> Result<(), String> {
    write_file(
        &target.join("assets/scenes/default.scene.json"),
        DEFAULT_SCENE,
    )?;
    write_file(&target.join("assets/scenes.toml"), SCENES_TOML)?;
    write_file(
        &target.join("assets/settings.toml"),
        &SETTINGS_TOML.replace("__NAME__", &toml_escape(name)),
    )?;
    write_file(&target.join(".gitignore"), GITIGNORE)?;

    if let Some(version) = editor_version.filter(|version| !version.trim().is_empty()) {
        write_project_version(target, version)?;
    }

    let Some(engine_root) = engine_root else {
        // 没有引擎源码（例如只装了打包好的 Hub）时仍然生成可被编辑器打开的
        // 资源工程；游戏 crate 留到有源码的机器上再建。
        write_file(&target.join("README.md"), NO_CRATE_README)?;
        return Ok(());
    };

    let package = crate_name(name);
    let ident = package.replace('-', "_");
    let cargo = CARGO_TOML
        .replace("__PACKAGE__", &package)
        .replace("__CRATE__", &ident)
        .replace(
            "__PRISM_APP__",
            &toml_path(&engine_root.join("crates/prism-app")),
        )
        .replace(
            "__PRISM_ENGINE__",
            &toml_path(&engine_root.join("crates/prism-engine")),
        )
        .replace(
            "__PRISM_SCENE__",
            &toml_path(&engine_root.join("crates/prism-scene")),
        );
    write_file(&target.join("Cargo.toml"), &cargo)?;
    write_file(
        &target.join("src/lib.rs"),
        &LIB_RS.replace("__NAME__", name),
    )?;
    write_file(
        &target.join("src/main.rs"),
        &MAIN_RS.replace("__CRATE__", &ident),
    )?;
    write_file(
        &target.join("README.md"),
        &README_MD.replace("__NAME__", name),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 模板
// ---------------------------------------------------------------------------

const DEFAULT_SCENE: &str = r#"{
  "version": 1,
  "entities": [
    {
      "name": "Skybox",
      "parent": null,
      "transform": {
        "translation": [0, 0, 0],
        "rotation": [0, 0, 0, 1],
        "scale": [1, 1, 1]
      },
      "components": {
        "prism_engine::scene::Skybox": {
          "enabled": false,
          "env_asset": 0,
          "hdr_path": "",
          "intensity": 1,
          "rotation_degrees": 0
        }
      }
    },
    {
      "name": "Camera",
      "parent": null,
      "transform": {
        "translation": [0, 2.5, 18],
        "rotation": [-0.06894800066947937, 0, 0, 0.9976199865341188],
        "scale": [1, 1, 1]
      },
      "components": {
        "prism_engine::scene::Camera": {
          "aspect": 1.777,
          "enabled": true,
          "exposure": 1,
          "far": 1000,
          "fov_y_degrees": 60,
          "near": 0.01
        }
      }
    },
    {
      "name": "Sun",
      "parent": null,
      "transform": {
        "translation": [0, 0, 0],
        "rotation": [0, 0, 0, 1],
        "scale": [1, 1, 1]
      },
      "components": {
        "prism_engine::scene::DirectionalLight": {
          "ambient": 1,
          "color": [1, 1, 1],
          "euler_xyz": [0, 0, 0],
          "intensity": 100000
        }
      }
    }
  ]
}
"#;

const SCENES_TOML: &str = r#"# PrismaRev 场景清单。
#
# `name` 是运行时查找键，`path` 是资源包内路径。
# 打包命令（在 PrismaRev 仓库根执行）：
#   cargo run -p prism-build-pipeline -- build-scenes \
#     --manifest <project>/assets/scenes.toml \
#     --staging <project>/target/scene_pack \
#     --output <project>/assets/scenes.pak

[[scenes]]
name = "default"
path = "scenes/default.rscn"
"#;

const SETTINGS_TOML: &str = r#"[app]
company = ""
name = "__NAME__"
version = "0.1.0"

[window]
title = "__NAME__"
width = 1280
height = 720
resizable = true
fullscreen = false
vsync = true

[render]
render_mode = "raster"
tonemap_mode = 0
debug_flags = 0
debug_rt = 0
pt_max_bounces = 3
pt_max_iterations = 0
pt_ray_max_distance = 1000.0

[build]
target = "windows"
profile = "release"
output_dir = "build/windows"
run_after = false
"#;

const GITIGNORE: &str = r#"/target/
/assets/scenes.pak
/assets/scenes.pak.meta.json
/build/
"#;

const CARGO_TOML: &str = r#"[package]
name = "__PACKAGE__"
version = "0.1.0"
edition = "2021"
description = "PrismaRev 用户工程"

# 独立 workspace：不挂进 PrismaRev 根 workspace。
[workspace]

[lib]
crate-type = ["lib", "cdylib"]

[dependencies]
prism-app = { path = "__PRISM_APP__" }
prism-engine = { path = "__PRISM_ENGINE__" }
prism-scene = { path = "__PRISM_SCENE__" }
log = "0.4"
"#;

const LIB_RS: &str = r#"//! __NAME__ 的游戏入口。

/// 构造应用：引擎负责窗口、渲染与音频；项目 system 可以在这里注册。
pub fn build_app() -> prism_app::App {
    prism_app::app()
        .with_subsystem(prism_app::Subsystem::Render)
        .with_subsystem(prism_app::Subsystem::Audio)
}
"#;

const MAIN_RS: &str = r#"#![deny(warnings)]

fn main() {
    __CRATE__::build_app().run();
}
"#;

const README_MD: &str = r#"# __NAME__

PrismaRev 用户工程。

- 用 PrismaRev Hub 打开本目录（或让编辑器以本目录为工作目录启动）。
- 场景在 `assets/scenes/`，运行时清单是 `assets/scenes.toml`。
- 编辑器版本记录在 `ProjectSettings/ProjectVersion.txt`，由 Hub 维护。
- 构建场景包（在 PrismaRev 仓库根执行）：

```sh
cargo run -p prism-build-pipeline -- build-scenes \
  --manifest <本工程>/assets/scenes.toml \
  --staging <本工程>/target/scene_pack \
  --output <本工程>/assets/scenes.pak
```

- 桌面游戏入口：`cargo run`（先构建场景包）。
"#;

const NO_CRATE_README: &str = r#"# 新工程

这是一个 PrismaRev 资源工程（`assets/`），可以直接用 PrismaRev Hub 打开。

当前 Hub 没有找到 PrismaRev 引擎源码，因此没有生成游戏 crate。要生成
`Cargo.toml` / `src/`，请先在 Hub 的设置页指定 PrismaRev 源码目录，再重新新建工程。
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("prisma-hub-project-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    #[test]
    fn version_file_parses_and_ignores_other_lines() {
        let text = "# comment\nm_EditorVersionWithRevision: 0.1.0 (abc)\nm_EditorVersion: 1.2.3\n";
        assert_eq!(parse_version_file(text).as_deref(), Some("1.2.3"));
        assert_eq!(parse_version_file(""), None);
        assert_eq!(parse_version_file("m_EditorVersion:   "), None);
    }

    #[test]
    fn version_marker_round_trips() {
        let root = scratch("version");
        assert!(read_project_version(&root).is_none());
        write_project_version(&root, "0.4.2").expect("写版本");
        assert_eq!(read_project_version(&root).as_deref(), Some("0.4.2"));
        let text = std::fs::read_to_string(root.join(VERSION_FILE)).expect("读文件");
        assert_eq!(text, "m_EditorVersion: 0.4.2\n");
    }

    #[test]
    fn empty_version_is_rejected() {
        let root = scratch("version-empty");
        assert!(write_project_version(&root, "  ").is_err());
        assert!(!root.join(VERSION_FILE).exists());
    }

    #[test]
    fn structure_reports_missing_pieces() {
        let root = scratch("structure");
        let structure = structure_of(&root);
        assert!(!structure.assets);
        assert!(!structure.scenes_dir);
        assert!(!structure.version_file);

        std::fs::create_dir_all(root.join("assets/scenes")).expect("创建 scenes");
        write_project_version(&root, "0.1.0").expect("写版本");
        let structure = structure_of(&root);
        assert!(structure.assets);
        assert!(structure.scenes_dir);
        assert!(structure.project_settings);
        assert!(structure.version_file);
        assert!(!structure.settings);
    }

    #[test]
    fn crate_names_are_cargo_safe() {
        assert_eq!(crate_name("My Game"), "my-game");
        assert_eq!(crate_name("钓鱼"), "game");
        assert_eq!(crate_name("2fast"), "game-2fast");
        assert_eq!(crate_name("---"), "game");
    }

    #[test]
    fn project_names_reject_paths_and_reserved_chars() {
        assert!(validate_project_name("My Game").is_ok());
        assert!(validate_project_name("钓鱼").is_ok());
        assert!(validate_project_name("").is_err());
        assert!(validate_project_name("..").is_err());
        assert!(validate_project_name("a/b").is_err());
        assert!(validate_project_name("a\\b").is_err());
        assert!(validate_project_name("bad?").is_err());
        assert!(validate_project_name("trailing.").is_err());
    }

    #[test]
    fn project_root_requires_a_scenes_directory() {
        let root = scratch("project-root");
        assert!(!is_project_root(&root));
        std::fs::create_dir_all(root.join("assets/scenes")).expect("创建 scenes");
        assert!(is_project_root(&root));
    }

    #[test]
    fn default_scene_is_valid_json() {
        let value: serde_json::Value =
            serde_json::from_str(DEFAULT_SCENE).expect("模板场景必须是合法 JSON");
        assert_eq!(value["version"], 1);
        assert!(value["entities"]
            .as_array()
            .is_some_and(|items| !items.is_empty()));
    }

    #[test]
    fn skeleton_without_engine_writes_assets_and_version() {
        let root = scratch("skeleton-no-engine");
        let target = root.join("demo");

        write_project_skeleton(&target, "Demo", None, Some("0.1.0")).expect("写资源骨架");

        assert!(is_project_root(&target));
        assert!(!target.join("Cargo.toml").exists());
        assert!(target.join("README.md").is_file());
        assert_eq!(read_project_version(&target).as_deref(), Some("0.1.0"));
        let structure = structure_of(&target);
        assert!(structure.scenes_manifest);
        assert!(structure.settings);
    }

    #[test]
    fn skeleton_with_engine_writes_cargo_and_sources() {
        let root = scratch("skeleton-engine");
        let engine = root.join("PrismaRev");
        let target = root.join("demo");

        write_project_skeleton(&target, "Demo Game", Some(&engine), Some("1.0.0"))
            .expect("写工程骨架");

        let cargo = std::fs::read_to_string(target.join("Cargo.toml")).expect("Cargo.toml");
        assert!(cargo.contains("name = \"demo-game\""), "{cargo}");
        assert!(
            cargo.contains(&toml_path(&engine.join("crates/prism-app"))),
            "{cargo}"
        );
        let main = std::fs::read_to_string(target.join("src/main.rs")).expect("main.rs");
        assert!(main.contains("demo_game::build_app()"), "{main}");
        let settings =
            std::fs::read_to_string(target.join("assets/settings.toml")).expect("settings");
        assert!(settings.contains("name = \"Demo Game\""), "{settings}");
        assert_eq!(read_project_version(&target).as_deref(), Some("1.0.0"));
        assert!(structure_of(&target).game_crate);
    }

    #[test]
    fn skeleton_without_editor_version_has_no_marker() {
        let root = scratch("skeleton-no-version");
        let target = root.join("demo");
        write_project_skeleton(&target, "Demo", None, None).expect("写骨架");
        assert!(!target.join(VERSION_FILE).exists());
        assert!(!structure_of(&target).version_file);
    }
}
