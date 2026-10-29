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
        <p>Hub 如何找到 PrismaRev 源码与编辑器</p>
      </div>
    </header>

    <div class="scroll-area">
      <div class="settings-card">
        <h2>PrismaRev 源码根</h2>
        <p>
          新建工程时用这里的 <code>crates/</code> 写游戏 crate 依赖，编辑器页也从这里找
          开发构建。<strong>不需要</strong>把 Hub 放进 PrismaRev 仓库。
        </p>

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

        <div class="hint">
          探测顺序：<code>PRISMREV_ROOT</code>（显式覆盖）→ 本页保存的路径 →
          Hub 所在目录的上级 → 同级目录里含 <code>crates/prism-app</code> 的
          checkout。环境变量设置了却无效会直接报错，不会静默回退。
        </div>
      </div>

      <div class="settings-card" style="margin-top: 16px">
        <h2>Hub 数据</h2>
        <p>
          工程列表、手动添加的编辑器、默认编辑器与源码根保存在一个 JSON 文件里；
          磁盘上的工程状态每次都重新探测，不缓存。
        </p>
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
