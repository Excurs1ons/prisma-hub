# PrismaRev Hub

PrismaRev 的桌面启动器，Unity Hub 形态：管理工程、管理编辑器安装、按工程记录的版本
打开编辑器。

Hub 是**独立项目**（自己的 Cargo workspace），不依赖 PrismaRev 仓库的内部布局；
它只通过「PrismaRev 源码根」定位引擎 crate 与编辑器。

## 功能

- **工程列表**：路径、最近打开时间、场景数、工程结构、记录用的编辑器版本。
- **新建工程**：生成 `assets/` 骨架（默认场景 + `scenes.toml` + `settings.toml`），
  找到源码时再生成最小游戏 crate（依赖用绝对路径，工程可以放在任意位置）。
- **版本识别**：每个工程用 `ProjectSettings/ProjectVersion.txt` 记录编辑器版本：
  ```text
  m_EditorVersion: 0.1.0
  ```
- **编辑器管理**：自动发现 + 手动添加，可设默认、可移除；版本通过
  `prisma-editor --version` 探测。
- **打开工程**：显式选择的编辑器 → 版本完全匹配的安装 → 默认编辑器。启动后把版本
  标记更新成实际使用的版本（Unity 的升级行为）。同一工程不会重复启动。
- **源码根设置**：`PRISMREV_ROOT` → 设置页保存的路径 → Hub 上级目录 → 同级目录自动发现。

## 定位规则

### PrismaRev 源码根

「源码根」必须同时包含 `crates/prism-app`、`crates/prism-engine` 与
`editor/src-tauri/tauri.conf.json`。解析顺序：

1. `PRISMREV_ROOT`（显式覆盖；设置了却无效会直接报错，不回退）；
2. 设置页保存的路径；
3. Hub 可执行文件的各级父目录（开发布局）；
4. Hub 可执行文件各级父目录的同级目录（例如 `~/src/PrismaRev` 与 `~/src/prisma-hub`）。

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
