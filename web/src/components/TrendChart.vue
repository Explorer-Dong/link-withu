<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { number } from "../state";
const props = defineProps({
  points: { type: Array, default: () => [] },
  label: { type: String, default: "访问趋势" },
});
const container = ref(null),
  width = ref(920);
let observer;
onMounted(() => {
  observer = new ResizeObserver(([entry]) => {
    width.value = Math.max(280, entry.contentRect.width);
  });
  observer.observe(container.value);
});
onUnmounted(() => observer?.disconnect());
const max = computed(() => Math.max(2, ...props.points.map((p) => p.count)));
const left = computed(() => Math.max(40, number(max.value).length * 8 + 12));
const xy = computed(() =>
  props.points.map((p, i) => ({
    ...p,
    x:
      left.value +
      (i / Math.max(props.points.length - 1, 1)) *
        (width.value - left.value - 30),
    y: 180 - (p.count / max.value) * 145,
  })),
);
const line = computed(() => xy.value.map((p) => `${p.x},${p.y}`).join(" "));
const area = computed(() =>
  xy.value.length
    ? `M${xy.value[0].x},180 L${line.value.replaceAll(" ", " L")} L${xy.value.at(-1).x},180 Z`
    : "",
);
const labels = computed(() =>
  xy.value.filter((p, i) =>
    [0, Math.floor((xy.value.length - 1) / 2), xy.value.length - 1].includes(i),
  ),
);
</script>
<template>
  <div ref="container" class="trend-chart">
    <svg
      :viewBox="`0 0 ${width} 220`"
      role="img"
      :aria-label="`${label}访问趋势，总计 ${points.reduce((n, p) => n + p.count, 0)} 次`"
    >
      <defs>
        <linearGradient id="trend-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="var(--accent)" stop-opacity=".18" />
          <stop offset="100%" stop-color="var(--accent)" stop-opacity=".01" />
        </linearGradient>
      </defs>
      <g v-for="step in [0, 1, 2]" :key="step">
        <line
          :x1="left"
          :x2="width - 30"
          :y1="35 + step * 72.5"
          :y2="35 + step * 72.5"
          stroke="var(--border)"
          stroke-dasharray="4 5"
        />
        <text
          :x="left - 12"
          :y="39 + step * 72.5"
          text-anchor="end"
          class="chart-label"
        >
          {{ number(Math.round(max * (1 - step / 2))) }}
        </text>
      </g>
      <path :d="area" fill="url(#trend-fill)" />
      <polyline
        :points="line"
        fill="none"
        stroke="var(--accent)"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <circle
        v-for="p in xy"
        :key="p.date"
        :cx="p.x"
        :cy="p.y"
        :r="points.length > 30 ? 2 : 3.5"
        fill="var(--surface)"
        stroke="var(--accent)"
        stroke-width="1.5"
      >
        <title>
          {{ p.date
          }}{{
            p.end_date && p.end_date !== p.date ? ` 至 ${p.end_date}` : ""
          }}
          · {{ number(p.count) }} 次访问
        </title>
      </circle>
      <text
        v-for="p in labels"
        :key="p.date"
        :x="p.x"
        y="210"
        text-anchor="middle"
        class="chart-label"
      >
        {{
          points[0]?.date.slice(0, 4) !==
          (points.at(-1)?.end_date || points.at(-1)?.date)?.slice(0, 4)
            ? p.date.slice(0, 7).replace("-", "/")
            : p.date.slice(5).replace("-", "/")
        }}
      </text>
    </svg>
  </div>
</template>
