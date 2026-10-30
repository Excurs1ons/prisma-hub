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
//!
//! ## 能力 → feature
//!
//! 新建工程时勾选的维度 / UI / 音频 / 物理 / 路径追踪 / 窗口后端会推导成 `prism-app`
//! 的 feature 列表（[`ProjectCapabilities::features`]），并决定模板场景里有哪些实体。
//! 生成规则只有这一处实现，Hub 的界面只展示 [`GeneratedSkeleton`] 回报的实际列表。

use serde::{Deserialize, Serialize};
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
pub fn entry(
    root: &Path,
    stored_path: &str,
    added_at: u64,
    last_opened: Option<u64>,
    running: bool,
) -> ProjectEntry {
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
// 能力 → feature / 场景
// ---------------------------------------------------------------------------

/// 工程维度：决定模板场景与组件组（`group-2d` / `group-3d`）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum Dimension {
    #[serde(rename = "2d")]
    TwoD,
    #[default]
    #[serde(rename = "3d")]
    ThreeD,
}

impl Dimension {
    fn label(self) -> &'static str {
        match self {
            Self::TwoD => "2D",
            Self::ThreeD => "3D",
        }
    }

    fn component_group(self) -> &'static str {
        match self {
            Self::TwoD => "group-2d",
            Self::ThreeD => "group-3d",
        }
    }
}

/// 新建工程时勾选的能力。字段名就是前端（`api.ts`）的 JSON 形状。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCapabilities {
    #[serde(default)]
    pub dimension: Dimension,
    #[serde(default)]
    pub ui: bool,
    #[serde(default)]
    pub audio: bool,
    #[serde(default)]
    pub physics: bool,
    #[serde(default)]
    pub pathtrace: bool,
    /// Linux 窗口后端（`prism-platform` 的 winit 后端；其它平台忽略）。
    #[serde(default = "default_window_backend")]
    pub x11: bool,
    #[serde(default = "default_window_backend")]
    pub wayland: bool,
}

fn default_window_backend() -> bool {
    true
}

impl Default for ProjectCapabilities {
    fn default() -> Self {
        Self {
            dimension: Dimension::default(),
            ui: false,
            audio: false,
            physics: false,
            pathtrace: false,
            x11: true,
            wayland: true,
        }
    }
}

impl ProjectCapabilities {
    /// 推导 `prism-app` 的 feature 列表（顺序固定，便于比较与展示）。
    ///
    /// - 维度 → `group-2d` / `group-3d`，总带 `group-ui`（组件组：场景里的
    ///   `Node` / `Style` / `Text` 仍然能反序列化）；
    /// - `ui` 是**渲染叠加 + 引擎 UI 系统**，与 `group-ui` 组件组是两件事；
    /// - 窗口后端只在勾选时追加。
    pub fn features(&self) -> Vec<String> {
        let mut features = vec![
            self.dimension.component_group().to_string(),
            "group-ui".to_string(),
        ];
        if self.ui {
            features.push("ui".to_string());
        }
        if self.audio {
            features.push("audio".to_string());
        }
        if self.physics {
            features.push("physics".to_string());
        }
        if self.pathtrace {
            features.push("pathtrace".to_string());
        }
        if self.x11 {
            features.push("x11".to_string());
        }
        if self.wayland {
            features.push("wayland".to_string());
        }
        features
    }

    /// Linux 上没有任何 winit 后端时生成的程序开不了窗口；其它平台没有这个限制。
    pub fn validate(&self) -> Result<(), String> {
        if cfg!(target_os = "linux") && !self.x11 && !self.wayland {
            return Err(
                "Linux 窗口后端至少要选 x11 或 wayland，否则生成的程序无法开窗口".to_string(),
            );
        }
        Ok(())
    }
}

/// 生成结果：`hub` 据此回报实际写进 `Cargo.toml` 的 feature 与生成了什么。
#[derive(Debug, Clone)]
pub struct GeneratedSkeleton {
    /// `prism-app` 的 feature 列表；没生成游戏 crate 时为空。
    pub features: Vec<String>,
    /// 是否生成了游戏 crate（找到引擎源码才有）。
    pub game_crate: bool,
}

// ---------------------------------------------------------------------------
// 新建工程骨架
// ---------------------------------------------------------------------------

/// 生成工程骨架。
///
/// - 资源骨架（场景 / 清单 / 设置 / `.gitignore`）按 `capabilities` 生成；
/// - `editor_version` 有值时写 `ProjectSettings/ProjectVersion.txt`；
/// - `engine_root` 有值时才生成游戏 crate（依赖用绝对路径，工程可以放在任意位置）。
pub fn write_project_skeleton(
    target: &Path,
    name: &str,
    engine_root: Option<&Path>,
    editor_version: Option<&str>,
    capabilities: &ProjectCapabilities,
) -> Result<GeneratedSkeleton, String> {
    capabilities.validate()?;

    write_file(
        &target.join("assets/scenes/default.scene.json"),
        &scene_json(capabilities, name),
    )?;
    write_file(&target.join("assets/scenes.toml"), SCENES_TOML)?;
    write_file(
        &target.join("assets/settings.toml"),
        &settings_toml(capabilities, name),
    )?;
    write_file(&target.join(".gitignore"), GITIGNORE)?;

    if let Some(version) = editor_version.filter(|version| !version.trim().is_empty()) {
        write_project_version(target, version)?;
    }

    let Some(engine_root) = engine_root else {
        // 没有引擎源码（例如只装了打包好的 Hub）时仍然生成可被编辑器打开的
        // 资源工程；游戏 crate 留到有源码的机器上再建。
        write_file(&target.join("README.md"), NO_CRATE_README)?;
        return Ok(GeneratedSkeleton {
            features: Vec::new(),
            game_crate: false,
        });
    };

    let features = capabilities.features();
    write_file(
        &target.join("Cargo.toml"),
        &cargo_toml(name, engine_root, &features),
    )?;
    write_file(&target.join("src/lib.rs"), &lib_rs(name, capabilities))?;
    write_file(&target.join("src/main.rs"), &main_rs(name))?;
    write_file(
        &target.join("README.md"),
        &readme_md(name, capabilities, &features),
    )?;
    Ok(GeneratedSkeleton {
        features,
        game_crate: true,
    })
}

fn cargo_toml(name: &str, engine_root: &Path, features: &[String]) -> String {
    let quoted = features
        .iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    CARGO_TOML
        .replace("__PACKAGE__", &crate_name(name))
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
        )
        .replace("__FEATURES__", &quoted)
}

fn lib_rs(name: &str, capabilities: &ProjectCapabilities) -> String {
    // 只请求真正编译进来的子系统：feature 没开时 `with_subsystem` 是空操作，
    // 写进模板反而会让人以为音频 / 物理已经在跑。
    let mut subsystems = String::from("\n        .with_subsystem(prism_app::Subsystem::Render)");
    if capabilities.ui {
        subsystems.push_str("\n        .with_subsystem(prism_app::Subsystem::UI)");
    }
    if capabilities.audio {
        subsystems.push_str("\n        .with_subsystem(prism_app::Subsystem::Audio)");
    }
    if capabilities.physics {
        subsystems.push_str("\n        .with_subsystem(prism_app::Subsystem::Physics)");
    }
    LIB_RS
        .replace("__NAME__", name)
        .replace("__SUBSYSTEMS__", &subsystems)
}

fn main_rs(name: &str) -> String {
    MAIN_RS.replace("__CRATE__", &crate_name(name).replace('-', "_"))
}

fn settings_toml(capabilities: &ProjectCapabilities, name: &str) -> String {
    let render_mode = if capabilities.pathtrace {
        "path_trace"
    } else {
        "raster"
    };
    SETTINGS_TOML
        .replace("__NAME__", &toml_escape(name))
        .replace("__RENDER_MODE__", render_mode)
}

fn readme_md(name: &str, capabilities: &ProjectCapabilities, features: &[String]) -> String {
    let mut chosen = vec![format!(
        "- 维度：{}（组件组 `{}`）",
        capabilities.dimension.label(),
        capabilities.dimension.component_group()
    )];
    if capabilities.ui {
        chosen.push("- UI 叠加：`ui`（渲染叠加 + 引擎 UI 系统）".to_string());
    }
    if capabilities.audio {
        chosen.push("- 音频：`audio`（`AudioEngine` + 音频解码线程）".to_string());
    }
    if capabilities.physics {
        chosen.push("- 物理：`physics`（Rapier 后台线程）".to_string());
    }
    if capabilities.pathtrace {
        chosen.push(
            "- 路径追踪：`pathtrace`（`assets/settings.toml` 里 `render_mode = \"path_trace\"`）"
                .to_string(),
        );
    }
    let mut backends = Vec::new();
    if capabilities.x11 {
        backends.push("x11");
    }
    if capabilities.wayland {
        backends.push("wayland");
    }
    if !backends.is_empty() {
        chosen.push(format!("- 窗口后端：{}", backends.join(" / ")));
    }
    README_MD
        .replace("__NAME__", name)
        .replace("__DIMENSION__", capabilities.dimension.label())
        .replace("__CAPABILITIES__", &chosen.join("\n"))
        .replace("__FEATURES__", &features.join(", "))
}

// ---------------------------------------------------------------------------
// 模板
// ---------------------------------------------------------------------------

/// 模板场景。`version: 1` 是引擎自己 `assets/scenes/*.scene.json` 的现状：
/// 缺 `id` 的文档由 loader 的 `migrate_scene_doc` 按数组顺序补齐。
#[derive(serde::Serialize)]
struct SceneDoc {
    version: u32,
    entities: Vec<serde_json::Value>,
}

fn scene_json(capabilities: &ProjectCapabilities, name: &str) -> String {
    let mut entities = Vec::new();
    match capabilities.dimension {
        Dimension::ThreeD => {
            entities.push(skybox_entity());
            entities.push(camera_entity(false));
            entities.push(sun_entity());
        }
        Dimension::TwoD => {
            entities.push(camera_entity(true));
            entities.push(sprite_entity());
        }
    }
    if capabilities.ui {
        entities.push(hud_entity(name));
    }
    let doc = SceneDoc {
        version: 1,
        entities,
    };
    let mut text = serde_json::to_string_pretty(&doc).expect("模板场景必须是可序列化 JSON");
    text.push('\n');
    text
}

fn transform(translation: [f64; 3], rotation: [f64; 4]) -> serde_json::Value {
    serde_json::json!({
        "translation": translation,
        "rotation": rotation,
        "scale": [1.0, 1.0, 1.0],
    })
}

fn skybox_entity() -> serde_json::Value {
    serde_json::json!({
        "name": "Skybox",
        "parent": null,
        "transform": transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]),
        "components": {
            "prism_engine::scene::Skybox": {
                "enabled": false,
                "env_asset": 0,
                "hdr_path": "",
                "intensity": 1.0,
                "rotation_degrees": 0.0,
            }
        }
    })
}

/// 2D 用正交相机 + 纯色清屏；3D 沿用「看向原点」的透视相机。
fn camera_entity(two_d: bool) -> serde_json::Value {
    let (translation, rotation) = if two_d {
        ([0.0, 0.0, 10.0], [0.0, 0.0, 0.0, 1.0])
    } else {
        (
            [0.0, 2.5, 18.0],
            [-0.068_948_000_669_479_37, 0.0, 0.0, 0.997_619_986_534_118_8],
        )
    };
    let mut camera = serde_json::json!({
        "aspect": 1.777,
        "enabled": true,
        "exposure": 1.0,
        "far": 1000.0,
        "fov_y_degrees": 60.0,
        "near": 0.01,
    });
    if two_d {
        camera["orthographic"] = serde_json::json!(true);
        camera["orthographic_size"] = serde_json::json!(5.0);
        camera["clear_flags"] = serde_json::json!("SolidColor");
        camera["background_color"] = serde_json::json!([0.05, 0.07, 0.1, 1.0]);
    }
    serde_json::json!({
        "name": "Camera",
        "parent": null,
        "transform": transform(translation, rotation),
        "components": { "prism_engine::scene::Camera": camera }
    })
}

fn sun_entity() -> serde_json::Value {
    serde_json::json!({
        "name": "Sun",
        "parent": null,
        "transform": transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]),
        "components": {
            "prism_engine::scene::DirectionalLight": {
                "ambient": 1.0,
                "color": [1.0, 1.0, 1.0],
                "euler_xyz": [0.0, 0.0, 0.0],
                "intensity": 100000.0,
            }
        }
    })
}

/// 2D 的占位精灵：`sprite_path` 为空 = 不绘制，填上贴图路径即可显示。
fn sprite_entity() -> serde_json::Value {
    serde_json::json!({
        "name": "Sprite",
        "parent": null,
        "transform": transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]),
        "components": {
            "prism_engine::scene::SpriteRenderer": {
                "sprite_path": "",
                "frame_size": [0, 0],
                "frame_index": 0,
                "pixels_per_unit": 100.0,
                "pivot": [0.5, 0.5],
                "color": [1.0, 1.0, 1.0, 1.0],
                "flip_x": false,
                "flip_y": false,
                "sorting_layer": 0,
                "order_in_layer": 0,
            }
        }
    })
}

/// 勾了 UI 叠加时的示例元素：左上角的半透明标签，证明 UI 系统已经接上。
fn hud_entity(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": "Hud",
        "parent": null,
        "transform": transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]),
        "components": {
            "prism_engine::ui::Node": {},
            "prism_engine::ui::Style": {
                "width": 320.0,
                "height": 44.0,
                "anchors": {"min_x": 0.0, "min_y": 0.0, "max_x": 0.0, "max_y": 0.0},
                "pivot": {"x": 0.0, "y": 0.0},
                "margin": {"left": 24.0, "right": 0.0, "top": 24.0, "bottom": 0.0},
                "background": [0.0, 0.0, 0.0, 0.45],
                "border_radius": 8.0,
                "visible": true,
            },
            "prism_engine::ui::Text": {
                "content": name,
                "font_size": 20.0,
                "color": [1.0, 1.0, 1.0, 1.0],
                "alignment": "Center",
            }
        }
    })
}

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
# path_trace 需要 prism-app 的 `pathtrace` feature；Hub 按勾选能力写好。
render_mode = "__RENDER_MODE__"
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
# feature 由 Hub 新建工程时勾选的能力推导；改能力就改这里的列表。
prism-app = { path = "__PRISM_APP__", default-features = false, features = [__FEATURES__] }
# 这两个也必须 default-features = false：否则 prism-scene 的默认组件组会把
# prism-app 的 group-* 开关重新打开。
prism-engine = { path = "__PRISM_ENGINE__", default-features = false }
prism-scene = { path = "__PRISM_SCENE__", default-features = false }
"#;

const LIB_RS: &str = r#"//! __NAME__ 的游戏入口。

/// 构造应用：引擎负责窗口与渲染；项目 system 可以在这里注册。
pub fn build_app() -> prism_app::App {
    prism_app::app()__SUBSYSTEMS__
}
"#;

const MAIN_RS: &str = r#"#![deny(warnings)]

fn main() {
    __CRATE__::build_app().run();
}
"#;

const README_MD: &str = r#"# __NAME__

PrismaRev 用户工程（__DIMENSION__）。

## 能力

Hub 按新建时勾选的能力生成了这些 `prism-app` feature：

```text
__FEATURES__
```

对应关系：

__CAPABILITIES__

`group-*` 是**组件组**（决定哪些组件类型注册、serde 代码是否进二进制），
`ui` / `audio` / `physics` / `pathtrace` 是**运行时能力**。改能力就改 `Cargo.toml` 里
`prism-app` 的 `features`，然后 `cargo check`。

## 目录

- `assets/scenes/`：作者场景（`.scene.json`），运行时清单是 `assets/scenes.toml`。
- `assets/settings.toml`：窗口 / 渲染 / 构建设置。
- `ProjectSettings/ProjectVersion.txt`：上次打开本工程的编辑器版本，由 Hub 维护。
- `src/lib.rs`：`build_app()`；`src/main.rs`：桌面入口。

## 运行

```sh
cargo run
```

先构建场景包（在 PrismaRev 仓库根执行）：

```sh
cargo run -p prism-build-pipeline -- build-scenes \
  --manifest <本工程>/assets/scenes.toml \
  --staging <本工程>/target/scene_pack \
  --output <本工程>/assets/scenes.pak
```
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

    fn caps_2d_ui() -> ProjectCapabilities {
        ProjectCapabilities {
            dimension: Dimension::TwoD,
            ui: true,
            ..ProjectCapabilities::default()
        }
    }

    fn caps_3d_ui_audio() -> ProjectCapabilities {
        ProjectCapabilities {
            dimension: Dimension::ThreeD,
            ui: true,
            audio: true,
            ..ProjectCapabilities::default()
        }
    }

    fn caps_2d_minimal() -> ProjectCapabilities {
        ProjectCapabilities {
            dimension: Dimension::TwoD,
            ..ProjectCapabilities::default()
        }
    }

    fn scene_of(caps: &ProjectCapabilities) -> serde_json::Value {
        serde_json::from_str(&scene_json(caps, "Demo")).expect("模板场景必须是合法 JSON")
    }

    fn entities_text(scene: &serde_json::Value) -> String {
        scene["entities"].to_string()
    }

    #[test]
    fn features_follow_the_capability_matrix() {
        assert_eq!(
            caps_2d_ui().features(),
            ["group-2d", "group-ui", "ui", "x11", "wayland"]
        );
        assert_eq!(
            caps_3d_ui_audio().features(),
            ["group-3d", "group-ui", "ui", "audio", "x11", "wayland"]
        );
        // 「极简」只有维度 + 默认的窗口后端；`group-ui` 是组件组，不是 UI 能力。
        assert_eq!(
            caps_2d_minimal().features(),
            ["group-2d", "group-ui", "x11", "wayland"]
        );
        let caps = ProjectCapabilities {
            physics: true,
            pathtrace: true,
            wayland: false,
            ..ProjectCapabilities::default()
        };
        assert_eq!(
            caps.features(),
            ["group-3d", "group-ui", "physics", "pathtrace", "x11"]
        );
    }

    #[test]
    fn capabilities_deserialize_from_the_frontend_shape() {
        let caps: ProjectCapabilities = serde_json::from_str(
            r#"{"dimension":"2d","ui":true,"audio":false,"physics":false,"pathtrace":false,"x11":true,"wayland":false}"#,
        )
        .expect("前端 JSON 必须能反序列化");
        assert_eq!(caps.dimension, Dimension::TwoD);
        assert!(caps.ui);
        assert!(!caps.wayland);

        // 缺字段时默认 3D + 两个窗口后端。
        let caps: ProjectCapabilities = serde_json::from_str("{}").expect("空对象也能反序列化");
        assert_eq!(caps.dimension, Dimension::ThreeD);
        assert!(caps.x11 && caps.wayland);
        assert!(!caps.ui && !caps.audio && !caps.physics && !caps.pathtrace);
    }

    #[test]
    fn linux_needs_at_least_one_window_backend() {
        let none = ProjectCapabilities {
            x11: false,
            wayland: false,
            ..ProjectCapabilities::default()
        };
        assert_eq!(none.validate().is_ok(), !cfg!(target_os = "linux"));
        assert!(ProjectCapabilities {
            x11: true,
            wayland: false,
            ..ProjectCapabilities::default()
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn three_d_scene_has_sky_camera_and_sun() {
        let value = scene_of(&ProjectCapabilities::default());
        assert_eq!(value["version"], 1);
        let text = entities_text(&value);
        assert!(text.contains("prism_engine::scene::Skybox"), "{text}");
        assert!(text.contains("prism_engine::scene::Camera"), "{text}");
        assert!(
            text.contains("prism_engine::scene::DirectionalLight"),
            "{text}"
        );
        assert!(!text.contains("SpriteRenderer"), "{text}");
        assert!(!text.contains("prism_engine::ui::Node"), "{text}");
    }

    #[test]
    fn two_d_scene_is_orthographic_without_3d_components() {
        let value = scene_of(&caps_2d_minimal());
        let text = entities_text(&value);
        assert!(
            text.contains("prism_engine::scene::SpriteRenderer"),
            "{text}"
        );
        assert!(!text.contains("Skybox"), "{text}");
        assert!(!text.contains("DirectionalLight"), "{text}");
        assert_eq!(
            value["entities"][0]["components"]["prism_engine::scene::Camera"]["orthographic"],
            true
        );
        assert_eq!(
            value["entities"][0]["components"]["prism_engine::scene::Camera"]["clear_flags"],
            "SolidColor"
        );
    }

    #[test]
    fn ui_capability_adds_a_hud_entity() {
        let value = scene_of(&caps_2d_ui());
        let text = entities_text(&value);
        assert!(text.contains("prism_engine::ui::Node"), "{text}");
        assert!(text.contains("prism_engine::ui::Style"), "{text}");
        assert!(text.contains("prism_engine::ui::Text"), "{text}");
        // 示例标签写工程名，方便一眼认出是哪个工程。
        assert_eq!(
            value["entities"][2]["components"]["prism_engine::ui::Text"]["content"],
            "Demo"
        );
    }

    #[test]
    fn settings_render_mode_tracks_pathtrace() {
        let raster = settings_toml(&caps_2d_minimal(), "Demo");
        assert!(raster.contains("render_mode = \"raster\""), "{raster}");

        let traced = settings_toml(
            &ProjectCapabilities {
                pathtrace: true,
                ..ProjectCapabilities::default()
            },
            "Demo",
        );
        assert!(traced.contains("render_mode = \"path_trace\""), "{traced}");
    }

    #[test]
    fn skeleton_without_engine_writes_assets_and_version() {
        let root = scratch("skeleton-no-engine");
        let target = root.join("demo");

        let generated = write_project_skeleton(&target, "Demo", None, Some("0.1.0"), &caps_2d_ui())
            .expect("写资源骨架");

        assert!(!generated.game_crate);
        assert!(generated.features.is_empty());
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

        let generated = write_project_skeleton(
            &target,
            "Demo Game",
            Some(&engine),
            Some("1.0.0"),
            &caps_3d_ui_audio(),
        )
        .expect("写工程骨架");

        assert!(generated.game_crate);
        assert_eq!(
            generated.features,
            ["group-3d", "group-ui", "ui", "audio", "x11", "wayland"]
        );

        let cargo = std::fs::read_to_string(target.join("Cargo.toml")).expect("Cargo.toml");
        assert!(cargo.contains("name = \"demo-game\""), "{cargo}");
        assert!(
            cargo.contains(&format!(
                "prism-app = {{ path = \"{}\", default-features = false, features = [",
                toml_path(&engine.join("crates/prism-app"))
            )),
            "{cargo}"
        );
        assert!(
            cargo.contains(
                "features = [\"group-3d\", \"group-ui\", \"ui\", \"audio\", \"x11\", \"wayland\"]"
            ),
            "{cargo}"
        );
        // 这两个默认特性会把组件组开关重新打开。
        assert!(
            cargo.contains(&format!(
                "prism-engine = {{ path = \"{}\", default-features = false }}",
                toml_path(&engine.join("crates/prism-engine"))
            )),
            "{cargo}"
        );
        assert!(
            cargo.contains(&format!(
                "prism-scene = {{ path = \"{}\", default-features = false }}",
                toml_path(&engine.join("crates/prism-scene"))
            )),
            "{cargo}"
        );

        let lib = std::fs::read_to_string(target.join("src/lib.rs")).expect("lib.rs");
        assert!(lib.contains("Subsystem::Render"), "{lib}");
        assert!(lib.contains("Subsystem::UI"), "{lib}");
        assert!(lib.contains("Subsystem::Audio"), "{lib}");
        assert!(!lib.contains("Subsystem::Physics"), "{lib}");

        let main = std::fs::read_to_string(target.join("src/main.rs")).expect("main.rs");
        assert!(main.contains("demo_game::build_app()"), "{main}");

        let settings =
            std::fs::read_to_string(target.join("assets/settings.toml")).expect("settings");
        assert!(settings.contains("name = \"Demo Game\""), "{settings}");

        let readme = std::fs::read_to_string(target.join("README.md")).expect("README");
        assert!(
            readme.contains("group-3d, group-ui, ui, audio, x11, wayland"),
            "{readme}"
        );
        assert!(readme.contains("3D"), "{readme}");

        assert_eq!(read_project_version(&target).as_deref(), Some("1.0.0"));
        assert!(structure_of(&target).game_crate);
    }

    #[test]
    fn minimal_project_requests_only_render_subsystem() {
        let root = scratch("skeleton-minimal");
        let engine = root.join("PrismaRev");
        let target = root.join("demo");

        let generated =
            write_project_skeleton(&target, "Demo", Some(&engine), None, &caps_2d_minimal())
                .expect("写工程骨架");

        assert_eq!(
            generated.features,
            ["group-2d", "group-ui", "x11", "wayland"]
        );
        let lib = std::fs::read_to_string(target.join("src/lib.rs")).expect("lib.rs");
        assert_eq!(lib.matches("with_subsystem").count(), 1, "{lib}");
        assert!(lib.contains("Subsystem::Render"), "{lib}");
    }

    #[test]
    fn skeleton_without_editor_version_has_no_marker() {
        let root = scratch("skeleton-no-version");
        let target = root.join("demo");
        write_project_skeleton(&target, "Demo", None, None, &caps_2d_minimal()).expect("写骨架");
        assert!(!target.join(VERSION_FILE).exists());
        assert!(!structure_of(&target).version_file);
    }

    #[test]
    fn invalid_capabilities_write_nothing() {
        let root = scratch("skeleton-invalid");
        let target = root.join("demo");
        let caps = ProjectCapabilities {
            x11: false,
            wayland: false,
            ..ProjectCapabilities::default()
        };
        let result = write_project_skeleton(&target, "Demo", None, None, &caps);
        if cfg!(target_os = "linux") {
            assert!(result.is_err());
            assert!(!target.exists());
        } else {
            assert!(result.is_ok());
        }
    }
}
