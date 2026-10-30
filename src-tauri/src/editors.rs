//! 编辑器安装的发现、版本探测与选择。
//!
//! Hub 不假设只有一个编辑器：
//!
//! - `env`：`PRISMREV_EDITOR_BIN` 显式指定（不可移除）；
//! - `bundled`：与 Hub 可执行文件同目录（打包布局）；
//! - `dev`：引擎 checkout 的 `editor/target/{debug,release}`（开发布局）；
//! - `manual`：用户在编辑器页手动添加的二进制（存进 `hub.json`）。
//!
//! 版本通过 `<binary> --version` 探测（输出 `prisma-editor <version>`）；探测失败时
//! 回退到二进制所在 checkout 的 `editor/Cargo.toml`，所以旧版编辑器
//! 没有 `--version` 也能显示版本。

use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::store::StoredEditor;
use crate::util::{canonical_string, profile, same_path};

pub const EDITOR_BIN_ENV: &str = "PRISMREV_EDITOR_BIN";

/// 编辑器二进制名：旧 Tauri 壳（打包 / cargo）与当前 egui 壳。
pub fn editor_binary_names() -> [String; 3] {
    [
        format!("prisma-editor{}", std::env::consts::EXE_SUFFIX),
        format!("prisma-editor-app{}", std::env::consts::EXE_SUFFIX),
        format!("prism-editor-egui{}", std::env::consts::EXE_SUFFIX),
    ]
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorInstall {
    /// 二进制绝对路径，同时作为 UI / 存储的稳定 id。
    pub path: String,
    pub version: String,
    /// `env` / `bundled` / `dev` / `manual`。
    pub source: String,
    pub available: bool,
    /// 用户手动添加的才能从列表移除。
    pub removable: bool,
    pub is_default: bool,
    /// 版本探测失败的说明（例如文件不可执行）。
    pub probe_error: Option<String>,
}

impl EditorInstall {
    fn new(path: &Path, source: &str) -> Self {
        Self {
            path: canonical_string(path),
            version: "unknown".to_string(),
            source: source.to_string(),
            available: path.is_file(),
            removable: source == "manual",
            is_default: false,
            probe_error: None,
        }
    }
}

/// 从 `prisma-editor 0.1.0` 这类输出里取出版本号。
///
/// 不引入正则：找第一个数字，从那里取到 token 末尾，只保留版本号允许的字符。
pub fn parse_version_output(output: &str) -> Option<String> {
    for token in output.split_whitespace() {
        let Some(start) = token.find(|ch: char| ch.is_ascii_digit()) else {
            continue;
        };
        let candidate: String = token[start..]
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+'))
            .collect();
        if candidate.contains('.') {
            return Some(candidate);
        }
    }
    None
}

/// 执行 `<binary> --version`。失败返回 `None`（调用方再走 tauri.conf.json 兜底）。
pub fn probe_version(binary: &Path) -> Result<String, String> {
    let output = Command::new(binary)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("无法执行 {}: {error}", binary.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_version_output(&stdout)
        .or_else(|| parse_version_output(&stderr))
        .ok_or_else(|| {
            format!(
                "{} --version 没有输出可识别的版本号（exit {:?}）",
                binary.display(),
                output.status.code()
            )
        })
}

/// 二进制在 `<engine>/editor/target/<profile>/` 下时，用 checkout 的版本兜底。
fn version_from_checkout(binary: &Path, engine_root: Option<&Path>) -> Option<String> {
    let root = engine_root?;
    let target = root.join("editor/target");
    if !binary.starts_with(&target) {
        return None;
    }
    crate::engine::editor_version(root)
}

/// 收集所有候选安装（未去重、未判定默认）。
pub fn candidates(
    exe_dir: &Path,
    engine_root: Option<&Path>,
    stored: &[StoredEditor],
    env_override: Option<&str>,
) -> Vec<EditorInstall> {
    let mut out = Vec::new();

    if let Some(raw) = env_override
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        out.push(EditorInstall::new(Path::new(raw), "env"));
    }

    let names = editor_binary_names();
    for name in &names {
        let path = exe_dir.join(name);
        if path.is_file() {
            out.push(EditorInstall::new(&path, "bundled"));
        }
    }

    if let Some(root) = engine_root {
        let target = root.join("editor/target");
        let other = if profile() == "debug" {
            "release"
        } else {
            "debug"
        };
        for build in [profile(), other] {
            for name in &names {
                let path = target.join(build).join(name);
                if path.is_file() {
                    out.push(EditorInstall::new(&path, "dev"));
                }
            }
        }
    }

    for editor in stored {
        let path = PathBuf::from(&editor.path);
        if !out
            .iter()
            .any(|existing| same_path(&existing.path, &canonical_string(&path)))
        {
            out.push(EditorInstall::new(&path, "manual"));
        }
    }

    out
}

/// 给安装补版本、去重、标记默认项。
///
/// `default` 是用户选择的路径；不可用时退回第一个可用安装（按候选顺序）。
pub fn resolve_installs(
    exe_dir: &Path,
    engine_root: Option<&Path>,
    stored: &[StoredEditor],
    default: Option<&str>,
    env_override: Option<&str>,
) -> Vec<EditorInstall> {
    let mut seen = HashSet::new();
    let mut installs: Vec<EditorInstall> = Vec::new();

    for mut install in candidates(exe_dir, engine_root, stored, env_override) {
        if !seen.insert(install.path.clone()) {
            continue;
        }
        if install.available {
            match probe_version(Path::new(&install.path)) {
                Ok(version) => install.version = version,
                Err(error) => {
                    if let Some(version) =
                        version_from_checkout(Path::new(&install.path), engine_root)
                    {
                        install.version = version;
                    } else {
                        install.probe_error = Some(error);
                    }
                }
            }
        }
        installs.push(install);
    }

    let default_index = default
        .and_then(|path| {
            installs
                .iter()
                .position(|install| same_path(&install.path, path) && install.available)
        })
        .or_else(|| installs.iter().position(|install| install.available));

    if let Some(index) = default_index {
        installs[index].is_default = true;
    }
    installs
}

/// 打开工程时要用的编辑器：显式指定 > 版本完全匹配 > 默认。
///
/// 返回 `(二进制路径, 版本)`；`None` 表示没有可用安装。
pub fn pick_for_project<'a>(
    installs: &'a [EditorInstall],
    explicit: Option<&str>,
    project_version: Option<&str>,
) -> Option<&'a EditorInstall> {
    if let Some(path) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return installs
            .iter()
            .find(|install| same_path(&install.path, path) && install.available);
    }
    if let Some(version) = project_version {
        if let Some(matched) = installs
            .iter()
            .find(|install| install.available && install.version == version)
        {
            return Some(matched);
        }
    }
    installs.iter().find(|install| install.is_default)
}

/// 显式指定但不可用时的错误信息，避免静默换成别的编辑器。
pub fn explicit_editor_error(installs: &[EditorInstall], path: &str) -> String {
    match installs
        .iter()
        .find(|install| same_path(&install.path, path))
    {
        Some(install) if !install.available => {
            format!("编辑器不存在或不可执行：{}", install.path)
        }
        Some(_) => format!("编辑器不可用：{path}"),
        None => format!("编辑器未在 Hub 中注册：{path}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("prisma-hub-editors-{name}-{}", std::process::id()));
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

    fn install(path: &str, version: &str, available: bool, default: bool) -> EditorInstall {
        EditorInstall {
            path: path.to_string(),
            version: version.to_string(),
            source: "dev".to_string(),
            available,
            removable: false,
            is_default: default,
            probe_error: None,
        }
    }

    #[test]
    fn parses_prisma_editor_version_output() {
        assert_eq!(
            parse_version_output("prisma-editor 0.1.0\n").as_deref(),
            Some("0.1.0")
        );
        assert_eq!(
            parse_version_output("prisma-editor v1.2.3-beta").as_deref(),
            Some("1.2.3-beta")
        );
        assert_eq!(parse_version_output("no version here"), None);
        assert_eq!(parse_version_output(""), None);
    }

    #[test]
    fn bundled_binaries_are_discovered() {
        let root = scratch("bundled");
        let exe_dir = root.join("dist");
        touch(&exe_dir.join(format!("prisma-editor{}", std::env::consts::EXE_SUFFIX)));

        let installs = resolve_installs(&exe_dir, None, &[], None, None);
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].source, "bundled");
        assert!(installs[0].available);
        assert!(installs[0].is_default);
    }

    #[test]
    fn dev_binaries_are_discovered_from_the_checkout() {
        let root = scratch("dev");
        let engine = root.join("PrismaRev");
        let exe_dir = root.join("hub/dist");
        std::fs::create_dir_all(&exe_dir).expect("创建 exe 目录");
        touch(
            &engine
                .join("editor/target/debug")
                .join(format!("prisma-editor-app{}", std::env::consts::EXE_SUFFIX)),
        );

        let installs = resolve_installs(&exe_dir, Some(&engine), &[], None, None);
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].source, "dev");
    }

    #[test]
    fn env_override_is_listed_even_when_missing() {
        let root = scratch("env");
        let exe_dir = root.join("dist");
        std::fs::create_dir_all(&exe_dir).expect("创建 exe 目录");
        let missing = root.join("bin/prisma-editor");

        let installs =
            resolve_installs(&exe_dir, None, &[], None, Some(&missing.to_string_lossy()));
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].source, "env");
        assert!(!installs[0].available);
        assert!(!installs[0].is_default);
    }

    #[test]
    fn manual_editors_are_removable_and_kept() {
        let root = scratch("manual");
        let exe_dir = root.join("dist");
        std::fs::create_dir_all(&exe_dir).expect("创建 exe 目录");
        let manual = root.join("custom/prisma-editor");
        touch(&manual);

        let stored = vec![StoredEditor {
            path: manual.to_string_lossy().into_owned(),
            added_at: 1,
        }];
        let installs = resolve_installs(&exe_dir, None, &stored, None, None);
        assert_eq!(installs.len(), 1);
        assert_eq!(installs[0].source, "manual");
        assert!(installs[0].removable);
        assert!(installs[0].is_default);
    }

    #[test]
    fn project_version_beats_the_default_editor() {
        let installs = vec![
            install("/bin/editor-1", "1.0.0", true, true),
            install("/bin/editor-2", "2.0.0", true, false),
        ];
        let picked = pick_for_project(&installs, None, Some("2.0.0")).expect("应匹配版本");
        assert_eq!(picked.path, "/bin/editor-2");
    }

    #[test]
    fn explicit_editor_wins_over_version_match() {
        let installs = vec![
            install("/bin/editor-1", "1.0.0", true, true),
            install("/bin/editor-2", "2.0.0", true, false),
        ];
        let picked =
            pick_for_project(&installs, Some("/bin/editor-1"), Some("2.0.0")).expect("应使用显式");
        assert_eq!(picked.path, "/bin/editor-1");
    }

    #[test]
    fn unknown_project_version_falls_back_to_default() {
        let installs = vec![install("/bin/editor-1", "1.0.0", true, true)];
        let picked = pick_for_project(&installs, None, Some("9.9.9")).expect("应回退默认");
        assert_eq!(picked.path, "/bin/editor-1");
    }

    #[test]
    fn unavailable_explicit_editor_is_reported() {
        let installs = vec![install("/bin/editor-1", "1.0.0", false, false)];
        let error = explicit_editor_error(&installs, "/bin/editor-1");
        assert!(error.contains("/bin/editor-1"), "{error}");
    }
}
