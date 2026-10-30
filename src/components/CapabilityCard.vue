<script setup lang="ts">
import { computed } from "vue";
import { capabilityPlatformLabel, type CapabilityInfo } from "../api";

/**
 * 新建工程向导里的一项能力 / 维度。
 *
 * 所有文字与数字都来自后端目录（`hub_capabilities`），组件不判断「哪些能力存在」，
 * 只负责把 `enabled: false` 的项置灰并显示原因。
 */
const props = withDefaults(
  defineProps<{
    entry: CapabilityInfo;
    checked: boolean;
    /** 利弊两列是否展开。 */
    expanded: boolean;
    /** 维度是单选组（`2D` / `3D`），其余是多选。 */
    radio?: boolean;
    /** 单选组的 `name`，让浏览器保证互斥。 */
    group?: string;
  }>(),
  { radio: false, group: "capability-dimension" },
);

const emit = defineEmits<{ change: [value: boolean]; expand: [] }>();

/** 成本徽章的色调：省体积 / 加体积 / 基准档。 */
const costTone = computed(() => {
  const delta = props.entry.sizeDeltaKb;
  if (delta === null || delta === 0) return "steady";
  return delta < 0 ? "save" : "add";
});

const platformText = computed(() =>
  props.entry.platforms.map(capabilityPlatformLabel).join(" · "),
);

/** 悬停提示补齐「完整说明」：一句话 + 利 + 弊（+ 置灰原因）。 */
const tooltip = computed(() => {
  const bullets = (items: string[]) => items.map((item) => `· ${item}`);
  const lines = [
    props.entry.summary,
    "",
    "利",
    ...bullets(props.entry.pros),
    "",
    "弊",
    ...bullets(props.entry.cons),
  ];
  if (!props.entry.enabled) {
    if (props.entry.disabledReason) {
      lines.push("", props.entry.disabledReason);
    }
    lines.push("", "当前引擎版本尚未接线，勾选不会写进 feature。");
  }
  return lines.join("\n");
});

function onToggle(event: Event) {
  emit("change", (event.target as HTMLInputElement).checked);
}
</script>

<template>
  <div
    class="option-card"
    :class="{ on: checked, off: !entry.enabled }"
    :title="tooltip"
  >
    <div class="option-head">
      <label class="option-label">
        <input
          v-if="radio"
          type="radio"
          :name="group"
          :checked="checked"
          :disabled="!entry.enabled"
          @change="onToggle"
        />
        <input
          v-else
          type="checkbox"
          :checked="checked"
          :disabled="!entry.enabled"
          @change="onToggle"
        />
        <span class="option-text">
          <strong>{{ entry.label }}</strong>
          <span class="option-summary">{{ entry.summary }}</span>
        </span>
      </label>
      <span class="badge cost" :class="costTone">{{ entry.cost }}</span>
      <span v-if="entry.platforms.length" class="badge platform">
        {{ platformText }}
      </span>
      <button
        type="button"
        class="option-more"
        :aria-expanded="expanded"
        @click="emit('expand')"
      >
        {{ expanded ? "收起" : "详情" }}
      </button>
    </div>

    <p v-if="!entry.enabled && entry.disabledReason" class="option-disabled">
      {{ entry.disabledReason }}
    </p>

    <div v-if="expanded" class="option-body">
      <div class="option-col">
        <span class="option-col-label pros">利</span>
        <ul>
          <li v-for="item in entry.pros" :key="item">{{ item }}</li>
        </ul>
      </div>
      <div class="option-col">
        <span class="option-col-label cons">弊</span>
        <ul>
          <li v-for="item in entry.cons" :key="item">{{ item }}</li>
        </ul>
      </div>
    </div>
  </div>
</template>
