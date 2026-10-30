import { invoke } from "@tauri-apps/api/core";

export type EngineSource = "env" | "setting" | "ancestor" | "sibling" | "none";

export type EngineInfo = {
  root: string | null;
  source: EngineSource;
  setting: string | null;
  editorVersion: string | null;
  hint: string | null;
};

export type EditorInstall = {
  path: string;
  version: string;
  source: "env" | "bundled" | "dev" | "manual";
  available: boolean;
  removable: boolean;
  isDefault: boolean;
  probeError: string | null;
};

export type ProjectStructure = {
  assets: boolean;
  scenesDir: boolean;
  scenesManifest: boolean;
  settings: boolean;
  gameCrate: boolean;
  projectSettings: boolean;
  versionFile: boolean;
};

export type ProjectEntry = {
  path: string;
  name: string;
  exists: boolean;
  valid: boolean;
  sceneCount: number;
  hasGameCrate: boolean;
  editorVersion: string | null;
  hasVersionFile: boolean;
  structure: ProjectStructure;
  lastOpened: number | null;
  addedAt: number;
  running: boolean;
};

export type HubEnvironment = {
  platform: "desktop";
  engine: EngineInfo;
  editors: EditorInstall[];
  defaultEditor: string | null;
  storePath: string;
};

/** 工程维度：决定组件组（`group-2d` / `group-3d`）。 */
export type ProjectDimension = "2d" | "3d";

/** 新建工程时勾选的能力；字段名与 Rust 的 `ProjectCapabilities` 一致。 */
export type ProjectCapabilities = {
  dimension: ProjectDimension;
  ui: boolean;
  audio: boolean;
  physics: boolean;
  pathtrace: boolean;
  /** Linux 窗口后端（其它平台忽略）。 */
  x11: boolean;
  wayland: boolean;
};

/** 新建工程的结果：`features` 是真实写进 `Cargo.toml` 的列表，由后端推导。 */
export type CreatedProject = {
  project: ProjectEntry;
  path: string;
  features: string[];
  gameCrate: boolean;
};

export const hub = {
  environment: () => invoke<HubEnvironment>("hub_environment"),
  setEngineRoot: (path: string) =>
    invoke<EngineInfo>("hub_set_engine_root", { path }),
  clearEngineRoot: () => invoke<EngineInfo>("hub_clear_engine_root"),
  editors: () => invoke<EditorInstall[]>("hub_editors"),
  addEditor: (path: string) => invoke<EditorInstall[]>("hub_add_editor", { path }),
  removeEditor: (path: string) =>
    invoke<EditorInstall[]>("hub_remove_editor", { path }),
  setDefaultEditor: (path: string) =>
    invoke<EditorInstall[]>("hub_set_default_editor", { path }),
  projects: () => invoke<ProjectEntry[]>("hub_projects"),
  addProject: (path: string) => invoke<ProjectEntry>("hub_add_project", { path }),
  removeProject: (path: string) =>
    invoke<ProjectEntry[]>("hub_remove_project", { path }),
  createProject: (
    name: string,
    location: string,
    capabilities: ProjectCapabilities,
  ) =>
    invoke<CreatedProject>("hub_create_project", {
      name,
      location,
      capabilities,
    }),
  openProject: (path: string, editor?: string | null) =>
    invoke<ProjectEntry>("hub_open_project", { path, editor: editor ?? null }),
  revealProject: (path: string) =>
    invoke<void>("hub_reveal_project", { path }),
};

export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}

export function relativeTime(seconds: number | null): string {
  if (!seconds) return "从未打开";
  const diff = Math.floor(Date.now() / 1000 - seconds);
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)} 天前`;
  return new Date(seconds * 1000).toLocaleDateString();
}

/** Windows 的 `\\?\C:\...` 长路径前缀在 UI 上不好看。 */
export function prettyPath(path: string): string {
  return path.replace(/^\\\\\?\\/, "");
}

export function editorSourceLabel(source: EditorInstall["source"]): string {
  switch (source) {
    case "env":
      return "环境变量";
    case "bundled":
      return "随 Hub 打包";
    case "dev":
      return "开发构建";
    case "manual":
      return "手动添加";
  }
}

export function engineSourceLabel(source: EngineSource): string {
  switch (source) {
    case "env":
      return "环境变量 PRISMREV_ROOT";
    case "setting":
      return "设置页指定";
    case "ancestor":
      return "Hub 所在目录的上级";
    case "sibling":
      return "同级目录自动发现";
    case "none":
      return "未找到";
  }
}
