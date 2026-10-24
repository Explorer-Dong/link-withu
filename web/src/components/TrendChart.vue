<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { number } from "../state";
const props = defineProps({
  points: { type: Array, default: () => [] },
  label: { type: String, default: "访问量趋势" },
});
const container = ref(null),
  width = ref(920),
  activeIndex = ref(null);
let observer;
onMounted(() => {
  observer = new ResizeObserver(([entry]) => {
    width.value = Math.max(1, entry.contentRect.width);
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
      props.points.length === 1
        ? (width.value + left.value - 30) / 2
        : left.value +
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
const activePoint = computed(() => xy.value[activeIndex.value] || null);
const tooltipStyle = computed(() => {
  const point = activePoint.value;
  if (!point) return {};
  const position = (point.x / width.value) * 100;
  const above = point.y > 110;
  return {
    left: `${position}%`,
    top: `${point.y + (above ? -16 : 16)}px`,
    transform: `translate(-${position}%, ${above ? "-100%" : "0"})`,
  };
});
const particles = Array.from({ length: 8 }, (_, n) => ({
  x: Math.cos((n * Math.PI) / 4) * (19 + (n % 3) * 5),
  y: Math.sin((n * Math.PI) / 4) * (19 + (n % 3) * 5),
}));
function hover(event) {
  const bounds = event.currentTarget.getBoundingClientRect();
  const x = ((event.clientX - bounds.left) * width.value) / bounds.width;
  const index = Math.round(
    ((x - left.value) / (width.value - left.value - 30)) *
      (xy.value.length - 1),
  );
  activeIndex.value = Math.max(0, Math.min(xy.value.length - 1, index));
}
function move(direction) {
  activeIndex.value = Math.max(
    0,
    Math.min(xy.value.length - 1, (activeIndex.value ?? 0) + direction),
  );
}
watch(
  () => props.points,
  () => (activeIndex.value = null),
);
</script>
<template>
  <div
    ref="container"
    class="trend-chart"
    tabindex="0"
    aria-label="访问量趋势，使用左右方向键查看日期"
    @keydown.right.prevent="move(1)"
    @keydown.left.prevent="move(-1)"
    @keydown.esc="activeIndex = null"
    @focus="activeIndex = 0"
    @blur="activeIndex = null"
  >
    <svg
      :viewBox="`0 0 ${width} 220`"
      role="img"
      :aria-label="`${label}，总计 ${points.reduce((n, p) => n + p.count, 0)} 次访问`"
      @pointermove="hover"
      @pointerdown="hover"
      @pointerleave="activeIndex = null"
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
        class="chart-point"
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
      <g
        v-if="activePoint"
        class="chart-active"
        :key="activePoint.date"
        aria-hidden="true"
      >
        <line
          :x1="activePoint.x"
          :x2="activePoint.x"
          y1="25"
          y2="180"
          stroke="var(--accent)"
          stroke-opacity=".35"
          stroke-dasharray="4 4"
        />
        <circle
          class="chart-point-halo"
          :cx="activePoint.x"
          :cy="activePoint.y"
          r="12"
          fill="var(--accent)"
          fill-opacity=".12"
        />
        <g :transform="`translate(${activePoint.x}, ${activePoint.y})`">
          <circle
            v-for="(particle, index) in particles"
            :key="index"
            class="chart-particle"
            cx="0"
            cy="0"
            :r="index % 2 ? 1.5 : 2.3"
            fill="var(--accent)"
            :style="{ '--dx': `${particle.x}px`, '--dy': `${particle.y}px` }"
          />
        </g>
        <circle
          :cx="activePoint.x"
          :cy="activePoint.y"
          r="5"
          fill="var(--accent)"
          stroke="var(--surface)"
          stroke-width="2"
        />
      </g>
    </svg>
    <div
      v-if="activePoint"
      role="tooltip"
      class="chart-tooltip"
      :style="tooltipStyle"
    >
      <span
        >{{ activePoint.date
        }}<template
          v-if="
            activePoint.end_date && activePoint.end_date !== activePoint.date
          "
        >
          至 {{ activePoint.end_date }}</template
        ></span
      >
      <strong
        ><i class="status-dot"></i>{{ number(activePoint.count)
        }}<small> 次访问</small></strong
      >
    </div>
  </div>
</template>
