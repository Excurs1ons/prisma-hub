<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  errorText,
  hub,
  type EditorInstall,
  type EngineInfo,
  type HubEnvironment,
  type ProjectEntry,
} from "./api";
import type { Notice, View } from "./types";
import ProjectsView from "./views/ProjectsView.vue";
import EditorsView from "./views/EditorsView.vue";
import SettingsView from "./views/SettingsView.vue";

const env = ref<HubEnvironment | null>(null);
const projects = ref<ProjectEntry[]>([]);
const ready = ref(false);
const bootError = ref("");
const view = ref<View>("projects");
const notice = ref<Notice | null>(null);

let noticeTimer: number | undefined;
let pollTimer: number | undefined;

const defaultEditor = computed(
  () => env.value?.editors.find((install) => install.isDefault) ?? null,
);
const availableEditors = computed(
  () => env.value?.editors.filter((install) => install.available).length ?? 0,
);

function notify(kind: Notice["kind"], text: string) {
  notice.value = { kind, text };
  if (noticeTimer !== undefined) window.clearTimeout(noticeTimer);
  noticeTimer = window.setTimeout(() => {
    notice.value = null;
  }, 4600);
}

async function refreshProjects(silent = false) {
  try {
    projects.value = await hub.projects();
  } catch (error) {
    if (!silent) notify("error", errorText(error));
  }
}

async function refreshEnvironment() {
  env.value = await hub.environment();
}

function onEditorsChanged(editors: EditorInstall[]) {
  if (!env.value) return;
  env.value = {
    ...env.value,
    editors,
    defaultEditor: editors.find((install) => install.isDefault)?.path ?? null,
  };
}

function onEngineChanged(engine: EngineInfo) {
  if (!env.value) return;
  env.value = { ...env.value, engine };
  // 引擎根变了，开发构建的编辑器候选也会变。
  void hub.editors().then(onEditorsChanged).catch(() => undefined);
}

onMounted(async () => {
  try {
    await refreshEnvironment();
    await refreshProjects();
    pollTimer = window.setInterval(() => void refreshProjects(true), 2500);
  } catch (error) {
    bootError.value = errorText(error);
  } finally {
    ready.value = true;
  }
});

onUnmounted(() => {
  if (pollTimer !== undefined) window.clearInterval(pollTimer);
  if (noticeTimer !== undefined) window.clearTimeout(noticeTimer);
});
</script>

<template>
  <div v-if="!ready" class="boot">
    <div class="boot-mark">P</div>
    <span>正在读取工程与编辑器…</span>
  </div>

  <main v-else-if="bootError" class="boot error-boot">
    <div class="boot-mark">!</div>
    <strong>Hub 启动失败</strong>
    <p>{{ bootError }}</p>
  </main>

  <div v-else-if="env" class="hub">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">P</div>
        <div class="brand-text">
          <strong>PrismaRev</strong>
          <span>Hub</span>
        </div>
      </div>

      <nav class="nav">
        <button
          class="nav-item"
          :class="{ active: view === 'projects' }"
          @click="view = 'projects'"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              d="M3 7.5A1.5 1.5 0 0 1 4.5 6H9l2 2h8.5A1.5 1.5 0 0 1 21 9.5v8A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5v-10Z"
            />
          </svg>
          <span>工程</span>
          <em>{{ projects.length }}</em>
        </button>
        <button
          class="nav-item"
          :class="{ active: view === 'editors' }"
          @click="view = 'editors'"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 5h16v11H4z" />
            <path d="M9 20h6M12 16v4" />
          </svg>
          <span>编辑器</span>
          <em>{{ availableEditors }}</em>
        </button>
        <button
          class="nav-item"
          :class="{ active: view === 'settings' }"
          @click="view = 'settings'"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <circle cx="12" cy="12" r="3" />
            <path
              d="M19.4 15a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.09A1.7 1.7 0 0 0 9 19.4a1.7 1.7 0 0 0-1.87.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.09A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.34-1.87l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.09A1.7 1.7 0 0 0 15 4.6a1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9c.14.6.66 1 1.55 1H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.51 1Z"
            />
          </svg>
          <span>设置</span>
        </button>
      </nav>

      <button
        class="sidebar-foot"
        :class="{ ok: defaultEditor }"
        @click="view = 'editors'"
      >
        <span class="status-dot"></span>
        <span class="foot-text">
          <strong>{{ defaultEditor ? "默认编辑器" : "未找到编辑器" }}</strong>
          <span>
            {{
              defaultEditor
                ? `v${defaultEditor.version} · ${defaultEditor.source === "manual" ? "手动添加" : "自动发现"}`
                : "点击添加安装"
            }}
          </span>
        </span>
      </button>
    </aside>

    <ProjectsView
      v-if="view === 'projects'"
      :projects="projects"
      :env="env"
      :notify="notify"
      @refresh="refreshProjects(false)"
    />
    <EditorsView
      v-else-if="view === 'editors'"
      :env="env"
      :notify="notify"
      @changed="onEditorsChanged"
    />
    <SettingsView
      v-else
      :env="env"
      :notify="notify"
      @engine-changed="onEngineChanged"
    />
  </div>

  <div v-if="notice" class="toast" :class="notice.kind">{{ notice.text }}</div>
</template>
