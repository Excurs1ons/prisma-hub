<script setup lang="ts">
import { ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import {
  engineSourceLabel,
  errorText,
  hub,
  prettyPath,
  type EngineInfo,
  type HubEnvironment,
} from "../api";
import type { Notice } from "../types";

const props = defineProps<{
  env: HubEnvironment;
  notify: (kind: Notice["kind"], text: string) => void;
}>();

const emit = defineEmits<{ "engine-changed": [engine: EngineInfo] }>();

const busy = ref(false);

async function chooseEngineRoot() {
  try {
    const picked = await open({
      directory: true,
      multiple: false,
      title: "选择 PrismaRev 源码根（包含 crates/ 与 editor/）",
    });
    if (typeof picked !== "string") return;
    busy.value = true;
    const engine = await hub.setEngineRoot(picked);
    emit("engine-changed", engine);
    props.notify("ok", "已更新 PrismaRev 源码根");
  } catch (error) {
    props.notify("error", errorText(error));
  } finally {
    busy.value = false;
  }
}

async function clearEngineRoot() {
  busy.value = true;
  try {
    const engine = await hub.clearEngineRoot();
    emit("engine-changed", engine);
    props.notify("ok", "已清除手动指定的源码根，改用自动探测");
  } catch (error) {
    props.notify("error", errorText(error));
  } finally {
    busy.value = false;
  }
}

async function revealStore() {
  try {
    const parent = props.env.storePath.replace(/[\\/][^\\/]+$/, "");
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
        <h1>设置</h1>
        <p>源码路径与本地数据</p>
      </div>
    </header>

    <div class="scroll-area">
      <div class="settings-card settings-engine">
        <h2>PrismaRev 源码根</h2>
        <p>用于新建工程和发现开发版编辑器。</p>

        <dl class="meta">
          <div>
            <dt>当前使用</dt>
            <dd class="mono">{{ env.engine.root ? prettyPath(env.engine.root) : "未找到" }}</dd>
          </div>
          <div>
            <dt>来源</dt>
            <dd>{{ engineSourceLabel(env.engine.source) }}</dd>
          </div>
          <div>
            <dt>手动指定</dt>
            <dd class="mono">{{ env.engine.setting ?? "—" }}</dd>
          </div>
          <div>
            <dt>编辑器版本</dt>
            <dd>{{ env.engine.editorVersion ?? "—" }}</dd>
          </div>
        </dl>

        <div class="field-row">
          <button class="btn primary" :disabled="busy" @click="chooseEngineRoot">
            选择源码目录…
          </button>
          <button
            class="btn"
            :disabled="busy || !env.engine.setting"
            @click="clearEngineRoot"
          >
            清除手动设置
          </button>
        </div>

        <pre v-if="env.engine.hint" class="hint mono">{{ env.engine.hint }}</pre>

      </div>

      <div class="settings-card settings-card-secondary settings-data">
        <h2>Hub 数据</h2>
        <p>工程列表、编辑器与源码路径。</p>
        <dl class="meta">
          <div>
            <dt>hub.json</dt>
            <dd class="mono">{{ prettyPath(env.storePath) }}</dd>
          </div>
        </dl>
        <button class="btn" @click="revealStore">在文件管理器中显示</button>
      </div>
    </div>
  </section>
</template>
