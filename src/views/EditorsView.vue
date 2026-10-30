<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { Monitor } from "@lucide/vue";
import {
  editorSourceLabel,
  engineSourceLabel,
  errorText,
  hub,
  prettyPath,
  type EditorInstall,
  type HubEnvironment,
} from "../api";
import type { Notice } from "../types";

const props = defineProps<{
  env: HubEnvironment;
  notify: (kind: Notice["kind"], text: string) => void;
}>();

const emit = defineEmits<{ changed: [editors: EditorInstall[]] }>();

const installs = ref<EditorInstall[]>([...props.env.editors]);
const busy = ref<string | null>(null);

watch(
  () => props.env.editors,
  (next) => {
    installs.value = [...next];
  },
);

const available = computed(() => installs.value.filter((install) => install.available));
const missing = computed(() => installs.value.filter((install) => !install.available));

function apply(next: EditorInstall[]) {
  installs.value = next;
  emit("changed", next);
}

async function refresh() {
  try {
    apply(await hub.editors());
    props.notify("ok", `已重新检测到 ${available.value.length} 个可用编辑器`);
  } catch (error) {
    props.notify("error", errorText(error));
  }
}

async function addEditor() {
  try {
    const picked = await open({
      directory: false,
      multiple: false,
      title: "选择 PrismaRev 编辑器可执行文件（prisma-editor.exe）",
    });
    if (typeof picked !== "string") return;
    busy.value = picked;
    const before = new Set(installs.value.map((install) => install.path));
    const next = await hub.addEditor(picked);
    apply(next);
    const added = next.find((install) => !before.has(install.path));
    props.notify("ok", `已添加编辑器${added ? ` v${added.version}` : ""}`);
  } catch (error) {
    props.notify("error", errorText(error));
  } finally {
    busy.value = null;
  }
}

async function setDefault(install: EditorInstall) {
  busy.value = install.path;
  try {
    apply(await hub.setDefaultEditor(install.path));
    props.notify("ok", `默认编辑器已切换为 v${install.version}`);
  } catch (error) {
    props.notify("error", errorText(error));
  } finally {
    busy.value = null;
  }
}

async function remove(install: EditorInstall) {
  if (!window.confirm(`从 Hub 移除编辑器 v${install.version}？不会删除磁盘文件。`)) {
    return;
  }
  busy.value = install.path;
  try {
    apply(await hub.removeEditor(install.path));
    props.notify("ok", "已移除编辑器");
  } catch (error) {
    props.notify("error", errorText(error));
  } finally {
    busy.value = null;
  }
}

async function reveal(install: EditorInstall) {
  try {
    const parent = install.path.replace(/[\\/][^\\/]+$/, "");
    await hub.revealProject(parent);
  } catch (error) {
    props.notify("error", errorText(error));
  }
}
</script>

<template>
  <section class="workspace">
    <header class="topbar">
      <div class="heading">
        <h1>编辑器</h1>
        <p>
          {{ available.length }} 可用 / {{ installs.length }} 已登记
          <span class="heading-source">引擎：{{ env.engine.root ? prettyPath(env.engine.root) : "未找到" }} · {{ engineSourceLabel(env.engine.source) }}</span>
        </p>
      </div>
      <div v-if="installs.length" class="actions">
        <button class="btn" @click="refresh">重新检测</button>
        <button class="btn primary" @click="addEditor">选择编辑器 .exe</button>
      </div>
    </header>

    <div class="scroll-area">
      <div class="editor-list" :class="{ 'is-empty': !installs.length }">
        <div
          v-for="install in installs"
          :key="install.path"
          class="editor-card"
          :class="{ default: install.isDefault, missing: !install.available }"
        >
          <div class="editor-head">
            <span class="status-dot" :class="{ ok: install.available }"></span>
            <div class="editor-title">
              <h2>
                v{{ install.version }}
                <span v-if="install.isDefault" class="badge default">默认</span>
                <span v-if="!install.available" class="badge missing">不可用</span>
                <span class="source-pill">{{ editorSourceLabel(install.source) }}</span>
              </h2>
              <p class="mono">{{ prettyPath(install.path) }}</p>
            </div>
            <div class="editor-actions">
              <button
                v-if="install.available && !install.isDefault"
                class="btn small"
                :disabled="busy === install.path"
                @click="setDefault(install)"
              >
                设为默认
              </button>
              <button
                class="btn small"
                :disabled="busy === install.path"
                @click="reveal(install)"
              >
                位置
              </button>
              <button
                v-if="install.removable"
                class="btn small danger ghost"
                :disabled="busy === install.path"
                @click="remove(install)"
              >
                移除
              </button>
            </div>
          </div>
          <pre v-if="install.probeError" class="probe-error mono">{{ install.probeError }}</pre>
        </div>

        <div v-if="!installs.length" class="empty-list editor-empty">
          <div class="empty-art">
            <Monitor :size="26" :stroke-width="1.4" aria-hidden="true" />
          </div>
          <strong>没有找到编辑器</strong>
          <p>选择已编译的 PrismaRev 编辑器可执行文件（.exe），不能选择源码目录。</p>
          <button class="btn primary" @click="addEditor">选择编辑器 .exe</button>
        </div>

        <div v-else-if="missing.length" class="hint">
          有 {{ missing.length }} 个安装指向的文件已不存在。移除它们，或恢复对应文件。
        </div>

      </div>
    </div>
  </section>
</template>
