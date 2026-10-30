# PrismaRev Hub

PrismaRev 的桌面启动器，Unity Hub 形态：管理工程、管理编辑器安装、按工程记录的版本
打开编辑器。

Hub 是**独立项目**（自己的 Cargo workspace），不依赖 PrismaRev 仓库的内部布局；
它只通过「PrismaRev 源码根」定位引擎 crate 与编辑器。

## 功能

- **工程列表**：路径、最近打开时间、场景数、工程结构、记录用的编辑器版本。
- **新建工程**：按勾选的能力生成模板——维度（2D / 3D）、UI 叠加、音频、物理、路径
  追踪、Linux 窗口后端（x11 / wayland）。`prism-app` 的 feature 由勾选项推导，模板场景
  也随维度变化（2D = 正交相机 + 精灵，3D = 天空盒 + 透视相机 + 平行光）；生成后界面显示
  实际路径与 feature 列表，失败时把 cargo/IO 错误留在弹窗里。找到源码时才生成游戏 crate
  （依赖用绝对路径，工程可以放在任意位置）。
- **版本识别**：每个工程用 `ProjectSettings/ProjectVersion.txt` 记录编辑器版本：
  ```text
  m_EditorVersion: 0.1.0
  ```
- **编辑器管理**：自动发现 + 手动添加，可设默认、可移除；版本通过
  `prisma-editor --version` 探测。
- **打开工程**：显式选择的编辑器 → 版本完全匹配的安装 → 默认编辑器。启动后把版本
  标记更新成实际使用的版本（Unity 的升级行为）。同一工程不会重复启动。
- **源码根设置**：`PRISMREV_ROOT` → 设置页保存的路径 → Hub 上级目录 → 同级目录自动发现。

## 新建工程的能力 → feature

勾选项推导成 `prism-app` 的 feature，规则只在 `src-tauri/src/projects.rs` 实现一次：

| 勾选 | feature |
| --- | --- |
| 维度 2D / 3D | `group-2d` / `group-3d`（总是同时带 `group-ui` 组件组） |
| UI 叠加 | `ui`（渲染叠加 + 引擎 UI 系统） |
| 音频 | `audio` |
| 物理 | `physics` |
| 路径追踪 | `pathtrace`（同时把 `assets/settings.toml` 的 `render_mode` 写成 `path_trace`） |
| Linux 窗口后端 | `x11` / `wayland`（可多选，默认全选；Linux 上至少要一个） |

`group-*` 是**组件组**（决定哪些组件类型注册），`ui` / `audio` / `physics` / `pathtrace`
是**运行时能力**，两者不是一回事。生成的 `Cargo.toml` 对 `prism-engine` 与 `prism-scene`
必须写 `default-features = false`，否则 `prism-scene` 的默认组件组会把 `group-*` 开关
重新打开。生成后的工程 `README.md` 会列出实际 feature 组合。

## 定位规则

### PrismaRev 源码根

「源码根」必须同时包含 `crates/prism-app`、`crates/prism-engine` 与
`editor/src-tauri/tauri.conf.json`。解析顺序：

1. `PRISMREV_ROOT`（显式覆盖；设置了却无效会直接报错，不回退）；
2. 设置页保存的路径；
3. Hub 可执行文件的各级父目录（开发布局）；
4. Hub 可执行文件各级父目录的同级目录（仅开发布局：Hub 自己是一个 checkout 时，
   例如 `~/src/PrismaRev` 与 `~/src/prisma-hub`）。

### 编辑器安装

不查 `PATH`：

1. `PRISMREV_EDITOR_BIN`（显式指定；不可从列表移除）；
2. 与 Hub 可执行文件同目录的 `prisma-editor[.exe]` / `prisma-editor-app[.exe]`；
3. `<源码根>/editor/src-tauri/target/{debug,release}/` 下的同名二进制；
4. 手动添加的安装（保存在 `hub.json`）。

版本探测失败时（旧版编辑器没有 `--version`），回退到该 checkout 的
`editor/src-tauri/tauri.conf.json` 版本。

## 开发

```sh
pnpm install
pnpm tauri dev
```

前端检查：

```sh
pnpm build
```

后端检查 / 测试（独立 workspace）：

```sh
cd src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
```

## 数据

`hub.json` 在 Tauri 的应用数据目录（设置页可以打开所在文件夹），只保存用户选择：
工程路径、手动添加的编辑器、默认编辑器、源码根。工程和编辑器的当前状态每次从磁盘
重新探测。

## 与 PrismaRev 的关系

PrismaRev 仓库里的 `launcher/` 只剩 Android APK 的启动壳（开始游戏 + `GameActivity`）。
桌面 Hub 在本仓库独立发布；Android 打包脚本仍由 PrismaRev 提供。

## 许可证

MIT OR Apache-2.0，见 [LICENSE-MIT](LICENSE-MIT) 与 [LICENSE-APACHE](LICENSE-APACHE)。
