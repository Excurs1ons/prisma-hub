<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import {
  editorSourceLabel,
  errorText,
  hub,
  prettyPath,
  relativeTime,
  type CreatedProject,
  type HubEnvironment,
  type ProjectCapabilities,
  type ProjectDimension,
  type ProjectEntry,
} from "../api";
import type { Notice } from "../types";

const props = defineProps<{
  projects: ProjectEntry[];
  env: HubEnvironment;
  notify: (kind: Notice["kind"], text: string) => void;
}>();

const emit = defineEmits<{ refresh: [] }>();

const selectedPath = ref<string | null>(null);
const filter = ref("");
const busy = ref<string | null>(null);
const editorChoice = ref("");

const showNew = ref(false);
const newName = ref("");
const newLocation = ref("");
const creating = ref(false);
/** 创建失败时的原始错误；留在弹窗里，用户不用去翻 toast。 */
const newError = ref("");
/** 创建成功后的结果：路径 + 后端实际写进 Cargo.toml 的 feature。 */
const created = ref<CreatedProject | null>(null);
const newDimension = ref<ProjectDimension>("3d");
const newUi = ref(false);
const newAudio = ref(false);
const newPhysics = ref(false);
const newPathtrace = ref(false);
const newX11 = ref(true);
const newWayland = ref(true);

/**
 * 勾选的能力。推导成 feature 的规则只在 Rust 侧
 * （`projects::ProjectCapabilities::features`），这里只负责收集。
 */
const newCapabilities = computed<ProjectCapabilities>(() => ({
  dimension: newDimension.value,
  ui: newUi.value,
  audio: newAudio.value,
  physics: newPhysics.value,
  pathtrace: newPathtrace.value,
  x11: newX11.value,
  wayland: newWayland.value,
}));

const selected = computed(
  () => props.projects.find((project) => project.path === selectedPath.value) ?? null,
);
const filteredProjects = computed(() => {
  const query = filter.value.trim().toLowerCase();
  if (!query) return props.projects;
  return props.projects.filter(
    (project) =>
      project.name.toLowerCase().includes(query) ||
      project.path.toLowerCase().includes(query),
  );
});
const editorOptions = computed(() =>
  props.env.editors.filter((install) => install.available),
);
const newProjectPath = computed(() => {
  const location = newLocation.value.trim().replace(/[\\/]+$/, "");
  const name = newName.value.trim();
  if (!location || !name) return "";
  return `${location}/${name}`;
});
const canCreate = computed(
  () => Boolean(newName.value.trim() && newLocation.value.trim()) && !creating.value,
);

/** 工程版本没有对应安装时，提示会用默认编辑器升级标记。 */
const versionMismatch = computed(() => {
  const project = selected.value;
  if (!project?.editorVersion) return null;
  const matched = editorOptions.value.find(
    (install) => install.version === project.editorVersion,
  );
  if (matched) return null;
  const fallback = editorOptions.value.find((install) => install.isDefault);
  return {
    project: project.editorVersion,
    fallback: fallback?.version ?? null,
  };
});

const chosenEditor = computed(
  () =>
    editorOptions.value.find((install) => install.path === editorChoice.value) ?? null,
);
const effectiveVersion = computed(
  () => chosenEditor.value?.version ?? props.env.editors.find((i) => i.isDefault)?.version,
);

watch(
  () => props.projects,
  (next) => {
    if (!next.some((project) => project.path === selectedPath.value)) {
      selectedPath.value = next[0]?.path ?? null;
    }
  },
  { immediate: true },
);

// 选中的编辑器被移除后回到「自动」，避免打开时报「未注册」。
watch(
  () => props.env.editors,
  (next) => {
    if (
      editorChoice.value &&
      !next.some((install) => install.path === editorChoice.value)
    ) {
      editorChoice.value = "";
    }
  },
);

async function openProject(project: ProjectEntry) {
  if (!project.valid || project.running || busy.value) return;
  busy.value = project.path;
  try {
    const updated = await hub.openProject(
      project.path,
      editorChoice.value || null,
    );
    selectedPath.value = updated.path;
    const version = updated.editorVersion ? ` · v${updated.editorVersion}` : "";
    props.notify("ok", `已启动 ${updated.name} 的编辑器${version}`);
    emit("refresh");
  } catch (error) {
    props.notify("error", errorText(error));
  } finally {
    busy.value = null;
  }
}

async function addProject() {
  try {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "选择 PrismaRev 工程目录（包含 assets/scenes）",
    });
    if (typeof picked !== "string") return;
    busy.value = picked;
    const entry = await hub.addProject(picked);
    selectedPath.value = entry.path;
    emit("refresh");
    props.notify("ok", `已添加工程 ${entry.name}`);
  } catch (error) {
    props.notify("error", errorText(error));
  } finally {
    busy.value = null;
  }
}

async function removeSelected() {
  const project = selected.value;
  if (!project) return;
  if (!window.confirm(`从 Hub 移除「${project.name}」？不会删除磁盘文件。`)) return;
  try {
    await hub.removeProject(project.path);
    emit("refresh");
    props.notify("ok", `已从列表移除 ${project.name}`);
  } catch (error) {
    props.notify("error", errorText(error));
  }
}

async function reveal(project: ProjectEntry) {
  try {
    await hub.revealProject(project.path);
  } catch (error) {
    props.notify("error", errorText(error));
  }
}

function openNewProject() {
  newName.value = "";
  newLocation.value = props.env.engine.root
    ? `${props.env.engine.root}/projects`
    : "";
  newDimension.value = "3d";
  newUi.value = false;
  newAudio.value = false;
  newPhysics.value = false;
  newPathtrace.value = false;
  newX11.value = true;
  newWayland.value = true;
  newError.value = "";
  created.value = null;
  showNew.value = true;
}

function closeNewProject() {
  showNew.value = false;
  created.value = null;
  newError.value = "";
}

async function chooseLocation() {
  try {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "选择工程父目录",
    });
    if (typeof picked === "string") newLocation.value = picked;
  } catch (error) {
    props.notify("error", errorText(error));
  }
}

async function createProject() {
  if (!canCreate.value) return;
  creating.value = true;
  newError.value = "";
  try {
    const result = await hub.createProject(
      newName.value.trim(),
      newLocation.value.trim(),
      newCapabilities.value,
    );
    created.value = result;
    selectedPath.value = result.project.path;
    emit("refresh");
    const features = result.features.length
      ? ` · ${result.features.join(", ")}`
      : "";
    props.notify("ok", `已创建工程 ${result.project.name}${features}`);
  } catch (error) {
    newError.value = errorText(error);
    props.notify("error", newError.value);
  } finally {
    creating.value = false;
  }
}

async function revealCreated() {
  if (!created.value) return;
  try {
    await hub.revealProject(created.value.path);
  } catch (error) {
    props.notify("error", errorText(error));
  }
}
</script>

<template>
  <section class="workspace">
    <header class="topbar">
      <div class="heading">
        <h1>工程</h1>
        <p>
          {{ projects.length }} 个工程 · 双击列表项直接打开 · 版本来自
          ProjectSettings/ProjectVersion.txt
        </p>
      </div>

      <div class="actions">
        <label class="search">
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <circle cx="11" cy="11" r="7" />
            <path d="m16 16 4 4" />
          </svg>
          <input v-model="filter" placeholder="搜索工程" spellcheck="false" />
        </label>
        <button class="btn" @click="addProject">添加</button>
        <button class="btn primary" @click="openNewProject">新建工程</button>
      </div>
    </header>

    <div class="projects">
      <div class="list">
        <div
          v-for="project in filteredProjects"
          :key="project.path"
          class="project-row"
          :class="{ selected: project.path === selectedPath }"
          tabindex="0"
          @click="selectedPath = project.path"
          @dblclick="openProject(project)"
          @keydown.enter="openProject(project)"
        >
          <div class="project-icon">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path
                d="M3 7.5A1.5 1.5 0 0 1 4.5 6H9l2 2h8.5A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z"
              />
            </svg>
          </div>
          <div class="project-main">
            <div class="project-title">
              <strong>{{ project.name }}</strong>
              <span v-if="project.editorVersion" class="badge version">
                {{ project.editorVersion }}
              </span>
              <span v-if="project.running" class="badge running">运行中</span>
              <span v-if="!project.exists" class="badge missing">丢失</span>
              <span v-else-if="!project.valid" class="badge warn">无效</span>
            </div>
            <span class="project-path mono">{{ prettyPath(project.path) }}</span>
          </div>
          <div class="project-side">
            <time>{{ relativeTime(project.lastOpened) }}</time>
            <span>{{ project.sceneCount }} 个场景</span>
          </div>
        </div>

        <div v-if="!projects.length" class="empty-list">
          <div class="empty-art">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path
                d="M3 7.5A1.5 1.5 0 0 1 4.5 6H9l2 2h8.5A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z"
              />
            </svg>
          </div>
          <strong>还没有工程</strong>
          <p>新建一个工程，或把已有的 PrismaRev 工程目录添加到 Hub。</p>
          <div class="empty-actions">
            <button class="btn" @click="addProject">添加已有工程</button>
            <button class="btn primary" @click="openNewProject">新建工程</button>
          </div>
        </div>

        <div v-else-if="!filteredProjects.length" class="empty-list compact">
          <strong>没有匹配「{{ filter }}」的工程</strong>
          <button class="link-btn" @click="filter = ''">清除搜索</button>
        </div>
      </div>

      <aside class="detail">
        <template v-if="selected">
          <div class="detail-head">
            <div class="project-icon large">
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path
                  d="M3 7.5A1.5 1.5 0 0 1 4.5 6H9l2 2h8.5A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z"
                />
              </svg>
            </div>
            <div class="detail-title">
              <h2>{{ selected.name }}</h2>
              <p class="mono">{{ prettyPath(selected.path) }}</p>
            </div>
          </div>

          <div class="detail-actions">
            <button
              class="btn primary block"
              :disabled="busy === selected.path || !selected.valid || selected.running"
              @click="openProject(selected)"
            >
              {{
                selected.running
                  ? "已在编辑器中打开"
                  : busy === selected.path
                    ? "正在启动…"
                    : "打开工程"
              }}
            </button>
            <button class="btn block" @click="reveal(selected)">
              在文件管理器中显示
            </button>
          </div>

          <label v-if="editorOptions.length" class="field">
            <span>使用编辑器</span>
            <select v-model="editorChoice">
              <option value="">自动（版本匹配 → 默认）</option>
              <option
                v-for="install in editorOptions"
                :key="install.path"
                :value="install.path"
              >
                v{{ install.version }} · {{ editorSourceLabel(install.source) }}
                {{ install.isDefault ? "· 默认" : "" }}
              </option>
            </select>
          </label>
          <p v-else class="callout warn">
            还没有可用编辑器。到「编辑器」页添加安装，或用
            <code>PRISMREV_EDITOR_BIN</code> 指定路径。
          </p>

          <div v-if="!selected.exists" class="callout danger">工程目录已丢失</div>
          <div v-else-if="!selected.valid" class="callout danger">
            缺少 assets/scenes，无法作为工程打开
          </div>
          <div v-else-if="versionMismatch" class="callout warn">
            工程记录的是 v{{ versionMismatch.project }}，
            {{
              versionMismatch.fallback
                ? `将用 v${versionMismatch.fallback} 打开并升级标记。`
                : "当前没有可用编辑器。"
            }}
          </div>

          <dl class="meta">
            <div>
              <dt>编辑器版本</dt>
              <dd>{{ selected.editorVersion ?? "未记录" }}</dd>
            </div>
            <div>
              <dt>打开时将使用</dt>
              <dd>
                {{
                  selected.running
                    ? "运行中"
                    : effectiveVersion
                      ? `v${effectiveVersion}`
                      : "—"
                }}
              </dd>
            </div>
            <div>
              <dt>场景</dt>
              <dd>{{ selected.sceneCount }} 个</dd>
            </div>
            <div>
              <dt>最近打开</dt>
              <dd>{{ relativeTime(selected.lastOpened) }}</dd>
            </div>
          </dl>

          <p class="section-label">工程结构</p>
          <div class="checklist">
            <span class="check-item" :class="{ ok: selected.structure.assets }">
              <i class="check-mark">{{ selected.structure.assets ? "✓" : "" }}</i>
              assets/
            </span>
            <span class="check-item" :class="{ ok: selected.structure.scenesDir }">
              <i class="check-mark">{{ selected.structure.scenesDir ? "✓" : "" }}</i>
              scenes/
            </span>
            <span
              class="check-item"
              :class="{ ok: selected.structure.scenesManifest }"
            >
              <i class="check-mark">{{
                selected.structure.scenesManifest ? "✓" : ""
              }}</i>
              scenes.toml
            </span>
            <span class="check-item" :class="{ ok: selected.structure.settings }">
              <i class="check-mark">{{ selected.structure.settings ? "✓" : "" }}</i>
              settings.toml
            </span>
            <span class="check-item" :class="{ ok: selected.structure.gameCrate }">
              <i class="check-mark">{{ selected.structure.gameCrate ? "✓" : "" }}</i>
              游戏 crate
            </span>
            <span
              class="check-item"
              :class="{ ok: selected.structure.projectSettings }"
            >
              <i class="check-mark">{{
                selected.structure.projectSettings ? "✓" : ""
              }}</i>
              ProjectSettings/
            </span>
            <span
              class="check-item"
              :class="{ ok: selected.structure.versionFile }"
            >
              <i class="check-mark">{{
                selected.structure.versionFile ? "✓" : ""
              }}</i>
              ProjectVersion.txt
            </span>
          </div>

          <button class="btn danger ghost block" @click="removeSelected">
            从列表移除
          </button>
        </template>

        <div v-else class="detail-empty">
          <strong>未选择工程</strong>
          <p>在左侧选择一个工程查看详情。</p>
        </div>
      </aside>
    </div>
  </section>

  <div v-if="showNew" class="modal-backdrop" @click.self="closeNewProject">
    <div class="modal">
      <header class="modal-head">
        <div>
          <h2>{{ created ? "工程已创建" : "新建工程" }}</h2>
          <p>
            {{
              created
                ? "下面是实际生成的目录与 feature 组合。"
                : "勾选能力后生成资源骨架与最小游戏 crate（feature 由勾选项推导）。"
            }}
          </p>
        </div>
        <button class="icon-btn" aria-label="关闭" @click="closeNewProject">×</button>
      </header>

      <template v-if="created">
        <div class="result-card">
          <p class="section-label">路径</p>
          <p class="result-path mono">{{ prettyPath(created.path) }}</p>

          <p class="section-label">实际 feature</p>
          <p v-if="created.features.length" class="feature-list">
            <code v-for="feature in created.features" :key="feature" class="feature-tag">
              {{ feature }}
            </code>
          </p>
          <p v-else class="result-note">
            没有生成游戏 crate（Hub 没找到 PrismaRev 源码），因此没有 feature 列表。
          </p>
        </div>

        <p v-if="!created.gameCrate" class="callout warn">
          只生成了资源工程。要补上 `Cargo.toml` / `src/`，先在「设置」页指定 PrismaRev
          源码目录，再重新新建工程。
        </p>
        <p v-else class="result-note">
          工程已加入列表。改能力 = 编辑 `Cargo.toml` 里 `prism-app` 的 features，再
          <code>cargo check</code>。
        </p>

        <footer class="modal-foot">
          <button class="btn" @click="revealCreated">在文件管理器中显示</button>
          <button class="btn primary" @click="closeNewProject">完成</button>
        </footer>
      </template>

      <template v-else>
        <label class="field">
          <span>工程名称</span>
          <input
            v-model="newName"
            placeholder="MyGame"
            spellcheck="false"
            autofocus
            @keydown.enter="createProject"
          />
        </label>

        <label class="field">
          <span>位置</span>
          <div class="field-row">
            <input v-model="newLocation" placeholder="选择工程父目录" spellcheck="false" />
            <button class="btn" @click="chooseLocation">浏览…</button>
          </div>
        </label>

        <p v-if="newProjectPath" class="path-preview mono">{{ newProjectPath }}</p>

        <div class="field">
          <span>维度</span>
          <div class="segmented">
            <button
              class="segment"
              :class="{ active: newDimension === '3d' }"
              @click="newDimension = '3d'"
            >
              3D
              <em>group-3d</em>
            </button>
            <button
              class="segment"
              :class="{ active: newDimension === '2d' }"
              @click="newDimension = '2d'"
            >
              2D
              <em>group-2d</em>
            </button>
          </div>
        </div>

        <div class="field">
          <span>能力</span>
          <div class="cap-grid">
            <label class="cap">
              <input v-model="newUi" type="checkbox" />
              <span>UI 叠加</span>
              <em>ui</em>
            </label>
            <label class="cap">
              <input v-model="newAudio" type="checkbox" />
              <span>音频</span>
              <em>audio</em>
            </label>
            <label class="cap">
              <input v-model="newPhysics" type="checkbox" />
              <span>物理</span>
              <em>physics</em>
            </label>
            <label class="cap">
              <input v-model="newPathtrace" type="checkbox" />
              <span>路径追踪</span>
              <em>pathtrace</em>
            </label>
          </div>
        </div>

        <div class="field">
          <span>Linux 窗口后端</span>
          <div class="cap-grid">
            <label class="cap">
              <input v-model="newX11" type="checkbox" />
              <span>x11</span>
            </label>
            <label class="cap">
              <input v-model="newWayland" type="checkbox" />
              <span>wayland</span>
            </label>
          </div>
        </div>

        <p v-if="!newX11 && !newWayland" class="callout warn">
          Linux 上至少要选一个窗口后端（x11 或 wayland），否则生成的程序无法开窗口。
          其它平台不需要这两个 feature。
        </p>

        <p v-if="!env.engine.root" class="callout warn">
          没有找到 PrismaRev 源码，只会生成资源工程（不含游戏 crate）。可以在「设置」页指定源码目录。
        </p>

        <p v-if="newError" class="callout danger">{{ newError }}</p>

        <footer class="modal-foot">
          <button class="btn" @click="closeNewProject">取消</button>
          <button class="btn primary" :disabled="!canCreate" @click="createProject">
            {{ creating ? "正在创建…" : "创建工程" }}
          </button>
        </footer>
      </template>
    </div>
  </div>
</template>
