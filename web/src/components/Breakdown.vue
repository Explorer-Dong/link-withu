<script setup>
import { computed } from "vue";
import { number } from "../state";
const props = defineProps({
  title: String,
  items: { type: Array, default: () => [] },
  total: { default: 0 },
});
defineEmits(["open"]);
const names = {
  unknown: "未知",
  desktop: "桌面设备",
  mobile: "手机",
  tablet: "平板",
};
const max = computed(() => Math.max(1, ...props.items.map((p) => p.count)));
</script>
<template>
  <section class="card breakdown">
    <div class="section-heading">
      <h2>
        <button
          class="section-title-button"
          :aria-label="title"
          @click="$emit('open')"
        >
          {{ title }}
        </button>
      </h2>
    </div>
    <div v-if="!items.length" class="mini-empty">暂无访问数据</div>
    <div v-for="item in items" :key="item.name" class="breakdown-row">
      <div>
        <span class="breakdown-name" :title="item.name">{{
          names[item.name] || item.name
        }}</span
        ><span
          ><strong>{{ number(item.count) }}</strong
          ><small
            >{{ total ? ((item.count / total) * 100).toFixed(1) : 0 }}%</small
          ></span
        >
      </div>
      <div class="bar-track">
        <div :style="{ width: `${(item.count / max) * 100}%` }"></div>
      </div>
    </div>
  </section>
</template>
