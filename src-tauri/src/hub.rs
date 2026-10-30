//! Hub 的进程内状态与 Tauri 命令。
//!
//! 状态只有三样东西：`hub.json` 里的用户选择、正在运行的编辑器子进程、以及每次
//! 从磁盘重新探测出来的工程 / 编辑器信息。工程和编辑器的「当前状态」永远不缓存，
//! 所以目录被移动、编辑器被删除时 UI 会立刻显示真实情况。

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

use crate::capabilities;
use crate::editors::{self, EditorInstall};
use crate::engine::{self, EngineInfo};
use crate::projects::{self, ProjectEntry};
use crate::store::{HubStore, StoredEditor, StoredProject, STORE_VERSION};
use crate::util::{
    canonical_dir, canonical_or_raw, canonical_string, current_exe_dir, now, same_path,
};

/// Hub 的进程内状态。
pub struct HubState {
    inner: Mutex<HubInner>,
}

struct HubInner {
    store_path: PathBuf,
    store: HubStore,
    /// 工程路径 → 编辑器子进程。只用来防止同一工程开两份，并给 UI 显示运行状态。
    editors: HashMap<String, Child>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubEnvironment {
    pub platform: &'static str,
    pub engine: EngineInfo,
    pub editors: Vec<EditorInstall>,
    /// 实际会使用的默认编辑器（可能来自自动发现，不一定是用户选的）。
    pub default_editor: Option<String>,
    pub store_path: String,
}

/// 新建工程的结果：界面要显示实际路径与真实下发的 feature 列表。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedProject {
    pub project: ProjectEntry,
    /// 工程根目录（创建后的 canonical 路径）。
    pub path: String,
    /// 写进 `Cargo.toml` 的 `prism-app` feature；没生成游戏 crate 时为空。
    pub features: Vec<String>,
    /// 是否生成了游戏 crate（没找到引擎源码时只有资源工程）。
    pub game_crate: bool,
}

impl HubState {
    /// 从应用数据目录加载；首次运行（没有 `hub.json`）时用引擎 checkout 里的工程做种子。
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("无法获取应用数据目录: {error}"))?;
        Self::load_at(&dir)
    }

    /// 在指定目录读写 `hub.json`。测试用这个入口，避免依赖 Tauri 的 `AppHandle`。
    pub fn load_at(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("无法创建应用数据目录 {}: {error}", dir.display()))?;
        let store_path = crate::store::store_path(dir);
        let (mut store, first_run) = HubStore::load(&store_path);
        store.version = STORE_VERSION;

        let inner = HubInner {
            store_path,
            store,
            editors: HashMap::new(),
        };
        let state = Self {
            inner: Mutex::new(inner),
        };

        if first_run {
            let engine = state.engine_info();
            let seeds = seed_projects(engine.root_path().as_deref());
            if !seeds.is_empty() {
                let mut inner = state.lock()?;
                inner.store.projects = seeds;
                inner.save()?;
            }
        }
        Ok(state)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, HubInner>, String> {
        self.inner.lock().map_err(|_| "Hub 状态已损坏".to_string())
    }

    fn engine_info(&self) -> EngineInfo {
        let setting = self
            .lock()
            .map(|inner| inner.store.engine_root.clone())
            .unwrap_or_default();
        let env = std::env::var(engine::ENGINE_ROOT_ENV).ok();
        engine::detect(&current_exe_dir(), setting.as_deref(), env.as_deref())
    }

    fn installs(&self, engine: &EngineInfo) -> Vec<EditorInstall> {
        let (stored, default) = self
            .lock()
            .map(|inner| {
                (
                    inner.store.editors.clone(),
                    inner.store.default_editor.clone(),
                )
            })
            .unwrap_or_default();
        let env = std::env::var(editors::EDITOR_BIN_ENV).ok();
        editors::resolve_installs(
            &current_exe_dir(),
            engine.root_path().as_deref(),
            &stored,
            default.as_deref(),
            env.as_deref(),
        )
    }

    pub fn environment(&self) -> HubEnvironment {
        let engine = self.engine_info();
        let installs = self.installs(&engine);
        let store_path = self
            .lock()
            .map(|inner| inner.store_path.to_string_lossy().into_owned())
            .unwrap_or_default();
        HubEnvironment {
            platform: "desktop",
            default_editor: installs
                .iter()
                .find(|install| install.is_default)
                .map(|install| install.path.clone()),
            engine,
            editors: installs,
            store_path,
        }
    }

    pub fn set_engine_root(&self, raw: &str) -> Result<EngineInfo, String> {
        let path = canonical_dir(raw)?;
        if !engine::is_engine_root(&path) {
            return Err(format!(
                "不是 PrismaRev 源码根（需要 crates/prism-app、crates/prism-engine 与 editor/src-tauri）：{}",
                path.display()
            ));
        }
        let mut inner = self.lock()?;
        inner.store.engine_root = Some(canonical_string(&path));
        inner.save()?;
        drop(inner);
        Ok(self.engine_info())
    }

    pub fn clear_engine_root(&self) -> Result<EngineInfo, String> {
        let mut inner = self.lock()?;
        inner.store.engine_root = None;
        inner.save()?;
        drop(inner);
        Ok(self.engine_info())
    }

    pub fn editors(&self) -> Vec<EditorInstall> {
        let engine = self.engine_info();
        self.installs(&engine)
    }

    pub fn add_editor(&self, raw: &str) -> Result<Vec<EditorInstall>, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("请选择编辑器可执行文件".to_string());
        }
        let path = PathBuf::from(trimmed)
            .canonicalize()
            .map_err(|error| format!("无法访问 {}: {error}", PathBuf::from(trimmed).display()))?;
        if !path.is_file() {
            return Err(format!("不是文件：{}", path.display()));
        }
        let path = canonical_string(&path);
        let mut inner = self.lock()?;
        if !inner
            .store
            .editors
            .iter()
            .any(|editor| same_path(&editor.path, &path))
        {
            inner.store.editors.push(StoredEditor {
                path: path.clone(),
                added_at: now(),
            });
        }
        // 第一个手动添加的编辑器自动成为默认，避免「加了却不用」。
        if inner.store.default_editor.is_none() {
            inner.store.default_editor = Some(path);
        }
        inner.save()?;
        drop(inner);
        Ok(self.editors())
    }

    pub fn remove_editor(&self, raw: &str) -> Result<Vec<EditorInstall>, String> {
        let key = canonical_or_raw(raw);
        let mut inner = self.lock()?;
        inner
            .store
            .editors
            .retain(|editor| !same_path(&editor.path, &key));
        if inner
            .store
            .default_editor
            .as_deref()
            .is_some_and(|default| same_path(default, &key))
        {
            inner.store.default_editor = None;
        }
        inner.save()?;
        drop(inner);
        Ok(self.editors())
    }

    pub fn set_default_editor(&self, raw: &str) -> Result<Vec<EditorInstall>, String> {
        let key = canonical_or_raw(raw);
        let installs = self.editors();
        let install = installs
            .iter()
            .find(|install| same_path(&install.path, &key))
            .ok_or_else(|| format!("编辑器未在 Hub 中注册：{raw}"))?;
        if !install.available {
            return Err(format!("编辑器不存在或不可执行：{}", install.path));
        }
        let mut inner = self.lock()?;
        inner.store.default_editor = Some(install.path.clone());
        inner.save()?;
        drop(inner);
        Ok(self.editors())
    }

    pub fn projects(&self) -> Result<Vec<ProjectEntry>, String> {
        Ok(self.lock()?.entries())
    }

    pub fn add_project(&self, raw: &str) -> Result<ProjectEntry, String> {
        let path = canonical_project_dir(raw)?;
        let key = canonical_string(&path);
        let mut inner = self.lock()?;

        if let Some(existing) = inner
            .store
            .projects
            .iter()
            .find(|project| same_path(&project.path, &key))
            .cloned()
        {
            return Ok(inner.entry_for(&existing));
        }

        inner.store.projects.push(StoredProject {
            path: key,
            added_at: now(),
            last_opened: None,
        });
        inner.save()?;
        let project = inner.store.projects.last().expect("刚插入").clone();
        Ok(inner.entry_for(&project))
    }

    pub fn remove_project(&self, raw: &str) -> Result<Vec<ProjectEntry>, String> {
        let key = canonical_or_raw(raw);
        let mut inner = self.lock()?;
        inner
            .store
            .projects
            .retain(|project| !same_path(&project.path, &key));
        inner.save()?;
        Ok(inner.entries())
    }

    /// 新建工程：目录 + `assets/` 骨架 + （能找到引擎源码时）按勾选能力生成的游戏
    /// crate，并把当前默认编辑器的版本写进 `ProjectSettings/ProjectVersion.txt`。
    ///
    /// 能力 → feature 的推导只在 [`projects::ProjectCapabilities::features`] 一处，
    /// 这里只负责把结果原样回报给界面。
    pub fn create_project(
        &self,
        name: &str,
        location: &str,
        capabilities: &projects::ProjectCapabilities,
    ) -> Result<CreatedProject, String> {
        let name = name.trim();
        projects::validate_project_name(name)?;
        capabilities.validate()?;
        let location = canonical_dir(location)?;
        let target = location.join(name);
        if target.exists() {
            return Err(format!("目标目录已存在：{}", target.display()));
        }

        let engine = self.engine_info();
        let installs = self.installs(&engine);
        let version = installs
            .iter()
            .find(|install| install.is_default)
            .map(|install| install.version.as_str())
            .filter(|version| *version != "unknown");

        std::fs::create_dir_all(&target)
            .map_err(|error| format!("无法创建工程目录 {}: {error}", target.display()))?;

        let result = projects::write_project_skeleton(
            &target,
            name,
            engine.root_path().as_deref(),
            version,
            capabilities,
        );
        let generated = match result {
            Ok(generated) => generated,
            Err(error) => {
                // 半成品目录比没有更糟：失败时整体回滚。
                let _ = std::fs::remove_dir_all(&target);
                return Err(error);
            }
        };

        let project = self.add_project(&target.to_string_lossy())?;
        Ok(CreatedProject {
            path: canonical_string(&target),
            project,
            features: generated.features,
            game_crate: generated.game_crate,
        })
    }

    /// 启动编辑器打开工程。
    ///
    /// 编辑器选择：`editor` 显式指定 > 版本标记完全匹配 > 默认编辑器。
    /// 启动后把版本标记更新成实际使用的版本（Unity 的升级行为）。
    pub fn open_project(&self, raw: &str, editor: Option<&str>) -> Result<ProjectEntry, String> {
        let path = canonical_project_dir(raw)?;
        let key = canonical_string(&path);

        let mut inner = self.lock()?;
        inner.prune_editors();
        if inner.editors.contains_key(&key) {
            return Err("该工程已在编辑器中打开".to_string());
        }
        drop(inner);

        let engine = self.engine_info();
        let installs = self.installs(&engine);
        let project_version = projects::read_project_version(&path);

        if let Some(requested) = editor.map(str::trim).filter(|value| !value.is_empty()) {
            if editors::pick_for_project(&installs, Some(requested), project_version.as_deref())
                .is_none()
            {
                return Err(editors::explicit_editor_error(&installs, requested));
            }
        }

        let install = editors::pick_for_project(&installs, editor, project_version.as_deref())
            .ok_or_else(|| {
                "没有可用的编辑器。在「编辑器」页添加安装，或用 PRISMREV_EDITOR_BIN 指定路径。"
                    .to_string()
            })?;

        let binary = PathBuf::from(&install.path);
        let mut command = Command::new(&binary);
        command.current_dir(&path).stdin(Stdio::null());
        let child = command
            .spawn()
            .map_err(|error| format!("启动编辑器 {} 失败：{error}", binary.display()))?;

        // 版本标记是「上次用哪个编辑器打开」的记录，启动成功后再写，失败不影响打开。
        if install.version != "unknown" {
            if let Err(error) = projects::write_project_version(&path, &install.version) {
                log::warn!("更新工程版本标记失败: {error}");
            }
        }

        let mut inner = self.lock()?;
        inner.editors.insert(key.clone(), child);
        if !inner
            .store
            .projects
            .iter()
            .any(|project| same_path(&project.path, &key))
        {
            inner.store.projects.push(StoredProject {
                path: key.clone(),
                added_at: now(),
                last_opened: None,
            });
        }
        if let Some(project) = inner
            .store
            .projects
            .iter_mut()
            .find(|project| same_path(&project.path, &key))
        {
            project.last_opened = Some(now());
        }
        inner.save()?;

        let project = inner
            .store
            .projects
            .iter()
            .find(|project| same_path(&project.path, &key))
            .cloned()
            .expect("刚插入或已存在");
        Ok(inner.entry_for(&project))
    }
}

impl HubInner {
    fn save(&self) -> Result<(), String> {
        self.store.save(&self.store_path)
    }

    /// 回收已经退出的编辑器，避免 UI 一直显示「运行中」。
    fn prune_editors(&mut self) {
        self.editors
            .retain(|_, child| matches!(child.try_wait(), Ok(None)));
    }

    fn entries(&mut self) -> Vec<ProjectEntry> {
        self.prune_editors();
        let mut entries: Vec<ProjectEntry> = self
            .store
            .projects
            .iter()
            .map(|project| self.entry_for(project))
            .collect();
        entries.sort_by(|a, b| {
            b.last_opened
                .cmp(&a.last_opened)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        entries
    }

    fn entry_for(&self, project: &StoredProject) -> ProjectEntry {
        let path = PathBuf::from(&project.path);
        projects::entry(
            &path,
            &project.path,
            project.added_at,
            project.last_opened,
            self.editors.contains_key(&project.path),
        )
    }
}

fn canonical_project_dir(raw: &str) -> Result<PathBuf, String> {
    let path = canonical_dir(raw)?;
    if !projects::is_project_root(&path) {
        return Err(format!(
            "不是 PrismaRev 工程（缺少 assets/scenes）：{}",
            path.display()
        ));
    }
    Ok(path)
}

/// 首次运行时的种子工程：引擎 checkout 根 + `projects/*` 里带 `assets/scenes` 的目录。
fn seed_projects(engine_root: Option<&Path>) -> Vec<StoredProject> {
    let Some(root) = engine_root else {
        return Vec::new();
    };
    let mut projects = Vec::new();
    push_seed(&mut projects, root.to_path_buf());
    if let Ok(entries) = std::fs::read_dir(root.join("projects")) {
        let mut children: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        children.sort();
        for child in children {
            push_seed(&mut projects, child);
        }
    }
    projects
}

fn push_seed(projects: &mut Vec<StoredProject>, path: PathBuf) {
    if !projects::is_project_root(&path) {
        return;
    }
    let path = canonical_string(&path);
    if projects
        .iter()
        .any(|project| same_path(&project.path, &path))
    {
        return;
    }
    projects.push(StoredProject {
        path,
        added_at: now(),
        last_opened: None,
    });
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn hub_environment(state: State<'_, HubState>) -> HubEnvironment {
    state.environment()
}

/// 新建工程的能力目录（说明 / 利弊 / 体积成本）。纯静态数据，不依赖 Hub 状态；
/// 前端不再硬编码任何一项。
#[tauri::command]
pub fn hub_capabilities() -> capabilities::CapabilityCatalog {
    capabilities::CATALOG
}

#[tauri::command]
pub fn hub_set_engine_root(state: State<'_, HubState>, path: String) -> Result<EngineInfo, String> {
    state.set_engine_root(&path)
}

#[tauri::command]
pub fn hub_clear_engine_root(state: State<'_, HubState>) -> Result<EngineInfo, String> {
    state.clear_engine_root()
}

#[tauri::command]
pub fn hub_editors(state: State<'_, HubState>) -> Vec<EditorInstall> {
    state.editors()
}

#[tauri::command]
pub fn hub_add_editor(
    state: State<'_, HubState>,
    path: String,
) -> Result<Vec<EditorInstall>, String> {
    state.add_editor(&path)
}

#[tauri::command]
pub fn hub_remove_editor(
    state: State<'_, HubState>,
    path: String,
) -> Result<Vec<EditorInstall>, String> {
    state.remove_editor(&path)
}

#[tauri::command]
pub fn hub_set_default_editor(
    state: State<'_, HubState>,
    path: String,
) -> Result<Vec<EditorInstall>, String> {
    state.set_default_editor(&path)
}

#[tauri::command]
pub fn hub_projects(state: State<'_, HubState>) -> Result<Vec<ProjectEntry>, String> {
    state.projects()
}

#[tauri::command]
pub fn hub_add_project(state: State<'_, HubState>, path: String) -> Result<ProjectEntry, String> {
    state.add_project(&path)
}

#[tauri::command]
pub fn hub_remove_project(
    state: State<'_, HubState>,
    path: String,
) -> Result<Vec<ProjectEntry>, String> {
    state.remove_project(&path)
}

#[tauri::command]
pub fn hub_create_project(
    state: State<'_, HubState>,
    name: String,
    location: String,
    capabilities: projects::ProjectCapabilities,
) -> Result<CreatedProject, String> {
    state.create_project(&name, &location, &capabilities)
}

#[tauri::command]
pub fn hub_open_project(
    state: State<'_, HubState>,
    path: String,
    editor: Option<String>,
) -> Result<ProjectEntry, String> {
    state.open_project(&path, editor.as_deref())
}

#[tauri::command]
pub fn hub_reveal_project(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return Err(format!("目录不存在：{}", path.display()));
    }
    reveal_dir(&path)
}

fn reveal_dir(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("explorer");
        command.arg(path);
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        command
            .spawn()
            .map_err(|error| format!("无法打开资源管理器: {error}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("无法打开 Finder: {error}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("无法打开文件管理器: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("prisma-hub-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建临时目录");
        dir
    }

    fn project_at(root: &Path) {
        std::fs::create_dir_all(root.join("assets/scenes")).expect("创建 scenes");
    }

    #[test]
    fn seed_collects_the_checkout_and_its_projects() {
        let root = scratch("seed");
        project_at(&root);
        project_at(&root.join("projects/demo"));
        std::fs::create_dir_all(root.join("projects/empty")).expect("创建空目录");

        let seeds = seed_projects(Some(&root));
        assert_eq!(seeds.len(), 2);
        assert!(seeds
            .iter()
            .any(|seed| seed.path.ends_with("projects/demo")));
    }

    #[test]
    fn seeds_are_empty_without_an_engine() {
        assert!(seed_projects(None).is_empty());
    }

    /// 写一个假的编辑器脚本：把调用参数记到 `trace`，并输出 `prisma-editor <version>`。
    #[cfg(unix)]
    fn fake_editor(dir: &Path, name: &str, version: &str, trace: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join(name);
        std::fs::create_dir_all(dir).expect("创建编辑器目录");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nprintf '%s %s\\n' \"$0\" \"$*\" >> \"{}\"\necho \"prisma-editor {version}\"\n",
                trace.display()
            ),
        )
        .expect("写假编辑器");
        let mut permissions = std::fs::metadata(&path).expect("读权限").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("设置可执行");
        path
    }

    #[cfg(unix)]
    fn wait_for(trace: &Path, matches: impl Fn(&str) -> bool) -> Vec<String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let lines: Vec<String> = std::fs::read_to_string(trace)
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect();
            if lines.iter().any(|line| matches(line)) {
                return lines;
            }
            if std::time::Instant::now() > deadline {
                panic!("等待编辑器启动超时，trace = {lines:#?}");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// 预先写一个空的 v2 配置，避免首次运行从当前机器上的引擎 checkout 播种工程。
    fn empty_store(dir: &Path) {
        std::fs::create_dir_all(dir).expect("创建数据目录");
        std::fs::write(dir.join(crate::store::STORE_FILE), r#"{"version":2}"#)
            .expect("写 hub.json");
    }

    #[cfg(unix)]
    #[test]
    fn opening_a_project_prefers_the_matching_editor_version() {
        let root = scratch("open-match");
        let data = root.join("data");
        empty_store(&data);

        let project = root.join("demo");
        project_at(&project);
        projects::write_project_version(&project, "2.0.0").expect("写版本");

        let trace = root.join("trace.log");
        let bin = root.join("bin");
        let v1 = fake_editor(&bin, "editor-1", "1.0.0", &trace);
        let v2 = fake_editor(&bin, "editor-2", "2.0.0", &trace);

        let state = HubState::load_at(&data).expect("加载状态");
        state.add_editor(&v1.to_string_lossy()).expect("添加 v1");
        state.add_editor(&v2.to_string_lossy()).expect("添加 v2");
        state
            .set_default_editor(&v1.to_string_lossy())
            .expect("默认 v1");

        let entry = state
            .open_project(&project.to_string_lossy(), None)
            .expect("打开工程");
        assert_eq!(entry.editor_version.as_deref(), Some("2.0.0"));

        wait_for(&trace, |line| {
            line.contains("editor-2") && !line.contains("--version")
        });
        // 版本一致时标记不变。
        assert_eq!(
            projects::read_project_version(&project).as_deref(),
            Some("2.0.0")
        );
    }

    #[cfg(unix)]
    #[test]
    fn opening_a_project_upgrades_the_marker_with_the_default_editor() {
        let root = scratch("open-upgrade");
        let data = root.join("data");
        empty_store(&data);

        let project = root.join("demo");
        project_at(&project);
        projects::write_project_version(&project, "0.9.0").expect("写旧版本");

        let trace = root.join("trace.log");
        let bin = root.join("bin");
        let editor = fake_editor(&bin, "editor-1", "1.0.0", &trace);

        let state = HubState::load_at(&data).expect("加载状态");
        state
            .add_editor(&editor.to_string_lossy())
            .expect("添加编辑器");

        let entry = state
            .open_project(&project.to_string_lossy(), None)
            .expect("打开工程");
        assert_eq!(entry.editor_version.as_deref(), Some("1.0.0"));
        assert_eq!(
            projects::read_project_version(&project).as_deref(),
            Some("1.0.0")
        );
        wait_for(&trace, |line| {
            line.contains("editor-1") && !line.contains("--version")
        });
    }

    #[cfg(unix)]
    #[test]
    fn explicit_missing_editor_is_an_error_not_a_fallback() {
        let root = scratch("open-explicit");
        let data = root.join("data");
        empty_store(&data);

        let project = root.join("demo");
        project_at(&project);

        let trace = root.join("trace.log");
        let bin = root.join("bin");
        let editor = fake_editor(&bin, "editor-1", "1.0.0", &trace);

        let state = HubState::load_at(&data).expect("加载状态");
        state
            .add_editor(&editor.to_string_lossy())
            .expect("添加编辑器");

        let error = state
            .open_project(&project.to_string_lossy(), Some("/nope/prisma-editor"))
            .expect_err("显式指定不存在的编辑器应报错");
        assert!(error.contains("/nope/prisma-editor"), "{error}");
    }
}
