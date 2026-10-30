# PrismaRev Hub — Agent Instructions

PrismaRev 的桌面工程管理器（Tauri 2 + Vue 3 + Rust）：管理工程、编辑器安装，并按
每个工程记录的版本打开编辑器。**独立仓库 / 独立 Cargo workspace**，不是 PrismaRev
根 workspace 的成员，也不依赖它的目录结构。

## 布局

- `src/` — Vue 3 前端。`App.vue` 只做外壳（侧栏 + 视图切换 + toast）；
  `views/ProjectsView.vue` / `views/EditorsView.vue` / `views/SettingsView.vue` 是三个页面。
  `api.ts` 是 Tauri 命令的**唯一**封装，类型必须与 Rust 的 `Serialize` 形状一致。
- `src/style.css` — 全部样式与颜色 token。组件里不要写死颜色，用 `var(--token)` 或
  `color-mix(in srgb, var(--token) N%, transparent)`。
- `src-tauri/src/`
  - `store.rs` — `hub.json`（v2）持久化。只存用户选择；磁盘状态不缓存。
  - `engine.rs` — PrismaRev 源码根定位（`PRISMREV_ROOT` → 设置 → 上级 → 同级）。
  - `editors.rs` — 编辑器安装发现、`--version` 探测、默认项、打开时选择。
  - `projects.rs` — 工程结构探测、`ProjectSettings/ProjectVersion.txt`、新建骨架
    （能力 → `prism-app` feature 的推导只在这里）。
  - `capabilities.rs` — 新建工程的能力目录：每一项的 `id` / 标题 / 一句话 / 利弊 /
    体积成本 / 默认值 / 平台 / 是否接线。**界面上的说明与数字只来自这里**。
  - `hub.rs` — 进程内状态 + Tauri 命令（薄封装）。
  - `util.rs` — 路径 / 时间小工具。

## 硬规则

- **不要**把 Hub 加进 PrismaRev 根 workspace；`src-tauri/Cargo.toml` 里的
  `[workspace]` 是故意的。
- **不要**按 `PATH` 找编辑器或游戏二进制；只按 `engine.rs` / `editors.rs` 的固定顺序。
  显式覆盖（环境变量 / UI 选择）指向不存在的文件时必须报错，不能静默回退。
- **工程版本只有一个来源**：`ProjectSettings/ProjectVersion.txt` 的
  `m_EditorVersion`。读写在 `projects.rs`；不要在别处再解析一遍。
- **打开工程的编辑器选择顺序**固定为：显式指定 → 版本完全匹配 → 默认。改顺序会改变
  「升级工程版本」的语义，先改 README 再改代码。
- 启动编辑器成功后写版本标记是**尽力而为**：失败只 `log::warn!`，不能让打开失败。
- **能力 → feature 只推导一次**：`projects.rs::ProjectCapabilities::features`。前端只收集
  勾选、只展示后端回报的列表；勾 UI 才带 `ui` + `group-ui`（不勾就两个都不加，别把 UI
  组件的 serde 链进极简构建）；生成的 `Cargo.toml` 必须给 `prism-engine` / `prism-scene`
  写 `default-features = false`（否则 `prism-scene` 的默认组件组会把 `group-*` 打开）。
- **能力目录也只有一个来源**：说明 / 利弊 / 体积成本 / 默认值只在
  `capabilities.rs`，前端通过 `hub_capabilities` 取，不许在 Vue 里硬编码选项。
  - 体积数字必须来自引擎侧实测；拿不到就写「未实测」，**不要估算完当实测**。
    `size_delta_kb` 要和 `cost` 原文的方向、量级一致，改数字要同步
    `size_deltas_match_the_measured_table` 测试。
  - `enabled: false` 的项要置灰并写清原因（引擎侧还没接线），不能隐藏。
  - 目录 id 与 `ProjectCapabilities` 的对应在 `option_state` / `apply_option`；新增一项时
    两处 + 目录 + `size_deltas_match_the_measured_table` 一起改，`cargo test` 会检查
    「每个 id 都能被推导识别」「置灰项不会被写进 feature」。
  - 「预计体积」是基准值 + 偏离基准项的增量，界面必须标**估算**并显示基准来源。
- `hub.json` 只存用户选择（工程路径 / 手动编辑器 / 默认编辑器 / 源码根）。任何「当前是否
  存在、版本号、场景数」都必须每次从磁盘重新探测。
- 新增命令：Rust 侧 `#[tauri::command]` + `lib.rs` 的 `generate_handler!` +
  `api.ts` 的类型与封装，三处一起改。
- 前端检查 `pnpm build`（`vue-tsc --noEmit` 必须过）；后端
  `cargo clippy --all-targets -- -D warnings` 必须过。

## 验证

```sh
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
pnpm build
```
