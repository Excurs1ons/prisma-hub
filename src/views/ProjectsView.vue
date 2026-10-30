<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import {
  editorSourceLabel,
  errorText,
  hub,
  prettyPath,
  relativeTime,
  targetPlatformFor,
  targetPlatformLabel,
  type CapabilityCatalog,
  type CapabilityInfo,
  type CreatedProject,
  type HubEnvironment,
  type PlatformBaseline,
  type ProjectCapabilities,
  type ProjectEntry,
  type SizeTierInfo,
  type TargetPlatform,
} from "../api";
import CapabilityCard from "../components/CapabilityCard.vue";
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
/** 能力目录（说明 / 利弊 / 成本）的唯一来源是后端，前端不硬编码任何一项。 */
const catalog = ref<CapabilityCatalog | null>(null);
const catalogError = ref("");
/** 向导的勾选状态：key 是目录项的 `id`，默认值来自目录的 `defaultOn`。 */
const newOptions = ref<Record<string, boolean>>({});
/** 体积档位（`standard` / `tiny`），与 2D/3D 同级。 */
const newTier = ref<"standard" | "tiny">("standard");
/** 展开了「利 / 弊」两列的目录项。 */
const expandedOptions = ref<Record<string, boolean>>({});

/** 目标平台 = Hub 自己运行的平台：Windows 专属项只在那时出现、也只在那时写进 feature。 */
const targetPlatform = computed<TargetPlatform>(() =>
  targetPlatformFor(props.env.hostPlatform),
);

/**
 * 勾选的能力。推导成 feature 的规则只在 Rust 侧
 * （`projects::ProjectCapabilities::features`），这里只负责把目录 id 映射成
 * 前端 JSON 形状。
 */
const newCapabilities = computed<ProjectCapabilities>(() => ({
  dimension: newOptions.value["2d"] ? "2d" : "3d",
  ui: Boolean(newOptions.value.ui),
  audio: Boolean(newOptions.value.audio),
  physics: Boolean(newOptions.value.physics),
  pathtrace: Boolean(newOptions.value.pathtrace),
  x11: Boolean(newOptions.value.x11),
  wayland: Boolean(newOptions.value.wayland),
  targetPlatform: targetPlatform.value,
  win32Native: Boolean(newOptions.value["win32-native"]),
  uiDirectwrite: Boolean(newOptions.value["ui-directwrite"]),
  sizeTier: newTier.value,
}));

/** 目录项是否适用于当前目标平台（空 `platforms` = 全平台）。 */
function appliesToTarget(entry: CapabilityInfo): boolean {
  return !entry.platforms.length || entry.platforms.includes(targetPlatform.value);
}

const dimensionOptions = computed(
  () =>
    catalog.value?.entries.filter(
      (entry) => entry.kind === "dimension" && appliesToTarget(entry),
    ) ?? [],
);
const capabilityOptions = computed(
  () =>
    catalog.value?.entries.filter(
      (entry) => entry.kind === "feature" && appliesToTarget(entry),
    ) ?? [],
);

/** 体积档位卡片复用 CapabilityCard 的排版；选项数据仍然来自后端目录。 */
const tierOptions = computed<CapabilityInfo[]>(() =>
  (catalog.value?.sizeTiers ?? []).map((tier) => ({
    id: tier.id,
    label: tier.label,
    summary: tier.summary,
    pros: tier.pros,
    cons: tier.cons,
    cost: tier.command,
    defaultOn: tier.defaultOn,
    platforms: [],
    enabled: true,
    kind: "feature",
    baselineOn: false,
    sizeDeltaKb: null,
    disabledReason: null,
  })),
);

type Estimate = {
  /** 最终估算（套过体积档位系数）。 */
  kb: number;
  /** 同一组合在标准发布档下的估算。 */
  standardKb: number;
  baselineKb: number;
  baselineLabel: string;
  baselineNote: string;
  deltas: { label: string; kb: number }[];
  tier: SizeTierInfo | null;
};

/**
 * 预计体积 = 平台基准 + 已经偏离基准的项的增量，再按体积档位系数折算。
 *
 * 数字全部来自目录（`baselines` / `baselineOn` / `sizeDeltaKb` / `sizeTiers`），与
 * Rust 侧 `capabilities.rs` 的测试是同一套算法。单项实测值相加不是严格线性的，
 * 极限体积档又只是把实测比例套到其它组合上，所以界面一律写「估算」并显示出处。
 */
const estimate = computed<Estimate | null>(() => {
  const data = catalog.value;
  if (!data) return null;
  const baseline: PlatformBaseline =
    data.baselines.find((item) => item.platform === targetPlatform.value) ??
    data.baselines[0];
  let standardKb = baseline.kb;
  const deltas: Estimate["deltas"] = [];
  for (const entry of data.entries) {
    if (!entry.enabled || !appliesToTarget(entry) || entry.sizeDeltaKb === null) continue;
    const on = Boolean(newOptions.value[entry.id]);
    if (on === entry.baselineOn) continue;
    standardKb += entry.sizeDeltaKb;
    if (entry.sizeDeltaKb !== 0) {
      deltas.push({
        label: `${on ? "开启" : "关闭"} ${entry.label}`,
        kb: entry.sizeDeltaKb,
      });
    }
  }
  const tier = data.sizeTiers.find((item) => item.id === newTier.value) ?? null;
  const percent = tier?.sizePercent ?? 100;
  return {
    kb: Math.round((standardKb * percent) / 100),
    standardKb,
    baselineKb: baseline.kb,
    baselineLabel: baseline.label,
    baselineNote: baseline.note,
    deltas,
    tier,
  };
});

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
  () =>
    Boolean(newName.value.trim() && newLocation.value.trim() && catalog.value) &&
    !creating.value,
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

/** 目录项的默认勾选状态；未接线的项、不适用当前平台的项永远是关的。 */
function catalogDefaults(): Record<string, boolean> {
  const next: Record<string, boolean> = {};
  for (const entry of catalog.value?.entries ?? []) {
    next[entry.id] =
      entry.enabled && entry.defaultOn && appliesToTarget(entry);
  }
  return next;
}

/** 目录里的默认体积档位（标准发布）。 */
function defaultTier(): "standard" | "tiny" {
  return catalog.value?.sizeTiers.find((tier) => tier.defaultOn)?.id ?? "standard";
}

async function loadCatalog() {
  try {
    catalog.value = await hub.capabilities();
    catalogError.value = "";
    if (!Object.keys(newOptions.value).length) {
      newOptions.value = catalogDefaults();
    }
  } catch (error) {
    catalogError.value = errorText(error);
  }
}

onMounted(() => {
  if (!catalog.value) void loadCatalog();
});

function setOption(id: string, on: boolean) {
  const next = { ...newOptions.value, [id]: on };
  // 与 Rust 侧的连带规则一致：DirectWrite 隐含 ui，关掉 ui 也关掉 DirectWrite。
  if (id === "ui-directwrite" && on) next.ui = true;
  if (id === "ui" && !on) next["ui-directwrite"] = false;
  newOptions.value = next;
}

/** 2D / 3D 是单选组：点一个就把同组其它项关掉。 */
function chooseDimension(id: string) {
  const next = { ...newOptions.value };
  for (const entry of dimensionOptions.value) {
    next[entry.id] = entry.id === id;
  }
  newOptions.value = next;
}

/** 体积档位是单选组。 */
function chooseTier(id: string) {
  newTier.value = id === "tiny" ? "tiny" : "standard";
}

function toggleDetails(id: string) {
  expandedOptions.value = {
    ...expandedOptions.value,
    [id]: !expandedOptions.value[id],
  };
}

function formatMb(kb: number): string {
  return `${(kb / 1000).toFixed(2)} MB`;
}

function formatDelta(kb: number): string {
  const sign = kb < 0 ? "−" : "+";
  const abs = Math.abs(kb);
  return abs >= 1000 ? `${sign}${(abs / 1000).toFixed(2)} MB` : `${sign}${abs} KB`;
}

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
  newOptions.value = catalogDefaults();
  newTier.value = defaultTier();
  expandedOptions.value = {};
  newError.value = "";
  if (!catalog.value) void loadCatalog();
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
    <div class="modal wide">
      <header class="modal-head">
        <div>
          <h2>{{ created ? "工程已创建" : "新建工程" }}</h2>
          <p>
            {{
              created
                ? "下面是实际生成的目录与 feature 组合。"
                : "勾选能力与体积档位后生成资源骨架与最小游戏 crate；每项都标了利弊与实测体积，feature 由后端推导。"
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

          <template v-if="estimate">
            <p class="section-label">预估体积</p>
            <p class="result-note">
              ≈ {{ formatMb(estimate.kb) }}（估算，来自引擎实测基线：
              {{ estimate.baselineLabel }}<template
                v-if="estimate.tier && estimate.tier.sizePercent !== 100"
              >；{{ estimate.tier.label }}按实测 −{{
                100 - estimate.tier.sizePercent
              }}% 折算</template>）。
            </p>
            <p
              v-if="estimate.tier && estimate.tier.sizePercent !== 100"
              class="result-note"
            >
              {{ estimate.tier.reference }}。代价：{{ estimate.tier.cons.join("；") }}。
            </p>
          </template>
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

        <p class="hint">
          目标平台：<strong>{{ targetPlatformLabel(targetPlatform) }}</strong>（Hub 当前系统）。
          Windows 专属项（Win32 原生窗口 / DirectWrite 文本）只在 Windows 目标下出现，
          Linux 的窗口后端只在 Linux 目标下出现；另一平台的选项不会写进 feature。
        </p>

        <p v-if="catalogError" class="callout danger">
          读取能力目录失败：{{ catalogError }}
          <button class="link-btn" @click="loadCatalog">重试</button>
        </p>

        <div class="field">
          <span>维度（互斥）</span>
          <div v-if="dimensionOptions.length" class="option-grid">
            <CapabilityCard
              v-for="entry in dimensionOptions"
              :key="entry.id"
              :entry="entry"
              :checked="Boolean(newOptions[entry.id])"
              :expanded="Boolean(expandedOptions[entry.id])"
              radio
              @change="chooseDimension(entry.id)"
              @expand="toggleDetails(entry.id)"
            />
          </div>
          <p v-else-if="!catalogError" class="option-loading">正在读取能力目录…</p>
        </div>

        <div class="field">
          <span>体积档位（互斥）</span>
          <div v-if="tierOptions.length" class="option-grid">
            <CapabilityCard
              v-for="entry in tierOptions"
              :key="entry.id"
              :entry="entry"
              :checked="newTier === entry.id"
              :expanded="Boolean(expandedOptions[entry.id])"
              radio
              group="capability-tier"
              @change="chooseTier(entry.id)"
              @expand="toggleDetails(entry.id)"
            />
          </div>
          <p v-else-if="!catalogError" class="option-loading">正在读取体积档位…</p>
        </div>

        <div class="field">
          <span>能力（可多选）</span>
          <div v-if="capabilityOptions.length" class="option-list">
            <CapabilityCard
              v-for="entry in capabilityOptions"
              :key="entry.id"
              :entry="entry"
              :checked="Boolean(newOptions[entry.id])"
              :expanded="Boolean(expandedOptions[entry.id])"
              @change="setOption(entry.id, $event)"
              @expand="toggleDetails(entry.id)"
            />
          </div>
          <p v-else-if="!catalogError" class="option-loading">正在读取能力目录…</p>
        </div>

        <div v-if="estimate" class="estimate">
          <div class="estimate-head">
            <span class="section-label">预计体积（估算，来自引擎实测基线）</span>
            <strong class="mono">≈ {{ formatMb(estimate.kb) }}</strong>
            <span class="badge">估算</span>
          </div>
          <p class="estimate-source">
            基准 {{ formatMb(estimate.baselineKb) }}：{{ estimate.baselineLabel }} ·
            {{ estimate.baselineNote }}
          </p>
          <p v-if="estimate.deltas.length" class="estimate-deltas">
            <span
              v-for="delta in estimate.deltas"
              :key="delta.label"
              class="estimate-delta"
            >
              {{ delta.label }} {{ formatDelta(delta.kb) }}
            </span>
          </p>
          <p v-else class="estimate-deltas">当前就是基准组合</p>
          <p
            v-if="estimate.tier && estimate.tier.sizePercent !== 100"
            class="estimate-source"
          >
            {{ estimate.tier.label }}：标准发布 {{ formatMb(estimate.standardKb) }} →
            {{ formatMb(estimate.kb) }}（按实测 −{{
              100 - estimate.tier.sizePercent
            }}% 折算，仍是估算）。出处：{{ estimate.tier.reference }}
          </p>
        </div>

        <p
          v-if="catalog && targetPlatform === 'linux' && !newOptions.x11 && !newOptions.wayland"
          class="callout warn"
        >
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
