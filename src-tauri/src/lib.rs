//! PrismaRev Hub —— Unity Hub 形态的桌面工程管理器。
//!
//! 模块划分：
//!
//! - [`store`]：`hub.json` 持久化（用户选择，不缓存磁盘状态）；
//! - [`engine`]：定位 PrismaRev 源码根（`PRISMREV_ROOT` → 设置 → 自动探测）；
//! - [`editors`]：编辑器安装发现、`--version` 探测、默认编辑器选择；
//! - [`projects`]：工程结构探测、`ProjectVersion.txt`、新建骨架；
//! - [`hub`]：进程内状态与 Tauri 命令。

mod editors;
mod engine;
mod hub;
mod projects;
mod store;
mod util;

use hub::{
    hub_add_editor, hub_add_project, hub_clear_engine_root, hub_create_project, hub_editors,
    hub_environment, hub_open_project, hub_projects, hub_remove_editor, hub_remove_project,
    hub_reveal_project, hub_set_default_editor, hub_set_engine_root, HubState,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = HubState::load(app.handle()).map_err(Box::<dyn std::error::Error>::from)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hub_environment,
            hub_set_engine_root,
            hub_clear_engine_root,
            hub_editors,
            hub_add_editor,
            hub_remove_editor,
            hub_set_default_editor,
            hub_projects,
            hub_add_project,
            hub_remove_project,
            hub_create_project,
            hub_open_project,
            hub_reveal_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PrismaRev Hub");
}
