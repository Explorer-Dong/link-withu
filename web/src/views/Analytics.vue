<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import Icon from "../components/Icon.vue";
import TrendChart from "../components/TrendChart.vue";
import Breakdown from "../components/Breakdown.vue";
import { api, getStats } from "../api/links";
import { tags, number, formatDate } from "../state";
const route = useRoute(),
  router = useRouter();
const days = ref(0),
  excludeBots = ref(false),
  data = ref(null),
  loading = ref(false),
  error = ref("");
const code = computed(() => route.query.short_code || ""),
  tag = computed(() => route.query.tag || "");
const visitPage = ref(1),
  visits = ref(null),
  visitsLoading = ref(false),
  visitsError = ref("");
let generation = 0,
  visitGeneration = 0;
const filters = computed(() => ({
  days: days.value,
  exclude_bots: excludeBots.value,
  short_code: code.value,
  tag: tag.value,
}));
const rangeLabel = computed(() =>
  days.value === 0 ? "全部时间" : `近 ${days.value} 天`,
);
const tagOptions = computed(() => {
  const available = [...tags.value];
  if (tag.value && !available.some((item) => item.name === tag.value))
    available.push({ name: tag.value });
  return available;
});
const metrics = computed(() =>
  data.value
    ? [
        {
          title: "今日访问",
          value: data.value.summary.today,
          icon: "eye",
          foot: "",
        },
        {
          title: "近 7 天",
          value: data.value.summary.last_7_days,
          icon: "chart",
          foot: "",
        },
        {
          title: "近 30 天",
          value: data.value.summary.last_30_days,
          icon: "clock",
          foot: "",
        },
        {
          title: "累计访问",
          value: data.value.summary.total_visits,
          icon: "link",
          foot: `共 ${number(data.value.summary.links)} 个链接`,
        },
      ]
    : [],
);
async function load() {
  const current = ++generation;
  loading.value = true;
  error.value = "";
  try {
    const [result, availableTags] = await Promise.all([
      api(`/api/analytics?${new URLSearchParams(filters.value)}`),
      api("/api/links/tags"),
    ]);
    if (current === generation) {
      data.value = result;
      tags.value = availableTags;
    }
  } catch (e) {
    if (current === generation) error.value = e.message;
  } finally {
    if (current === generation) loading.value = false;
  }
}
async function loadVisits() {
  const current = ++visitGeneration;
  if (!code.value) {
    visits.value = null;
    return;
  }
  visitsLoading.value = true;
  visitsError.value = "";
  try {
    const result = await getStats(code.value, {
      skip: (visitPage.value - 1) * 50,
      limit: 50,
      exclude_bots: excludeBots.value,
    });
    if (current === visitGeneration) visits.value = result;
  } catch (e) {
    if (current === visitGeneration) visitsError.value = e.message;
  } finally {
    if (current === visitGeneration) visitsLoading.value = false;
  }
}
function setTag(value) {
  router.replace({
    path: "/",
    query: { ...route.query, tag: value || undefined },
  });
}
watch(filters, load);
watch([code, excludeBots], () => {
  visitPage.value = 1;
  loadVisits();
});
watch(visitPage, loadVisits);
onMounted(() => {
  load();
  loadVisits();
});
onUnmounted(() => {
  generation++;
  visitGeneration++;
});
</script>
<template>
  <div class="page-heading">
    <div><h1>访问统计</h1></div>
  </div>
  <div class="analytics-toolbar">
    <div class="filters">
      <div class="segmented" aria-label="统计时间范围">
        <button
          v-for="n in [0, 7, 30, 90]"
          :key="n"
          :class="{ selected: days === n }"
          @click="days = n"
        >
          {{ n === 0 ? "全部时间" : `近 ${n} 天` }}
        </button>
      </div>
      <select
        v-if="tagOptions.length"
        :value="tag"
        aria-label="统计标签"
        @change="setTag($event.target.value)"
      >
        <option value="">全部标签</option>
        <option v-for="t in tagOptions" :key="t.name" :value="t.name">
          # {{ t.name }}
        </option>
      </select>
    </div>
    <label class="checkbox-label"
      ><input v-model="excludeBots" type="checkbox" />排除疑似机器人</label
    >
  </div>
  <div v-if="code" class="filter-notice">
    <span><Icon name="link" :size="16" />当前短链：{{ code }}</span
    ><button class="text-btn" @click="router.push('/')">
      查看全部链接 <Icon name="close" :size="16" />
    </button>
  </div>
  <p v-if="error" class="error-message" role="alert">
    {{ error }} <button class="text-btn" @click="load">重试</button>
  </p>
  <div v-if="loading" class="loading-state" aria-live="polite">
    正在汇总访问数据…
  </div>
  <template v-else-if="data && !error"
    ><div class="metrics-grid">
      <section
        v-for="metric in metrics"
        :key="metric.title"
        class="card metric-card"
      >
        <div>
          <span>{{ metric.title }}</span
          ><Icon :name="metric.icon" :size="18" />
        </div>
        <strong>{{ number(metric.value) }}</strong
        ><small v-if="metric.foot">{{ metric.foot }}</small>
      </section>
    </div>
    <section class="card trend-card">
      <div class="section-heading">
        <div>
          <h2>访问趋势</h2>
          <p class="muted small">
            {{ rangeLabel }}，共 {{ number(data.summary.period_visits) }} 次访问
          </p>
        </div>
        <span class="chart-legend"><span class="status-dot"></span>访问量</span>
      </div>
      <TrendChart :points="data.trend" :label="rangeLabel" />
      <p class="muted small chart-footnote">
        疑似机器人：{{ number(data.summary.suspected_bots) }} 次{{
          excludeBots ? "（已排除）" : ""
        }}
      </p>
    </section>
    <div class="breakdowns-grid">
      <Breakdown
        title="访问来源"
        :items="data.referrers"
        :total="data.summary.period_visits"
      /><Breakdown
        title="设备分布"
        :items="data.devices"
        :total="data.summary.period_visits"
      /><Breakdown
        title="浏览器"
        :items="data.browsers"
        :total="data.summary.period_visits"
      />
    </div>
    <div class="analytics-bottom-grid">
      <section class="card ranking">
        <div class="section-heading">
          <h2>热门短链</h2>
          <span class="muted small">TOP 10 · {{ rangeLabel }}</span>
        </div>
        <div v-if="!data.top_links.length" class="mini-empty">暂无访问数据</div>
        <router-link
          v-for="(link, i) in data.top_links"
          :key="link.short_code"
          :to="{
            path: '/',
            query: { ...route.query, short_code: link.short_code },
          }"
          class="ranking-row"
          ><span class="rank-number">{{ String(i + 1).padStart(2, "0") }}</span
          ><span class="rank-link"
            ><strong>{{ link.title || link.short_code }}</strong
            ><small>{{ link.original_url }}</small></span
          ><strong>{{ number(link.count) }}<small> 次</small></strong
          ><Icon name="arrow" :size="16"
        /></router-link>
      </section>
      <section class="card ranking">
        <div class="section-heading">
          <h2>标签分组</h2>
          <span class="muted small">{{ rangeLabel }}</span>
        </div>
        <div v-if="!data.tags.length" class="mini-empty">暂无标签数据</div>
        <button
          v-for="t in data.tags"
          :key="t.name"
          class="group-row"
          @click="setTag(t.name)"
        >
          <span class="tag-pill"># {{ t.name }}</span
          ><span class="muted small">{{ t.link_count }} 个链接</span
          ><strong>{{ number(t.count) }}<small> 次</small></strong>
        </button>
      </section>
    </div>
  </template>
  <section v-if="code" class="card visit-details">
    <div class="section-heading">
      <div>
        <h2>访问明细</h2>
        <p class="muted small">全部历史</p>
      </div>
      <span class="muted small">{{ number(visits?.total_visits) }} 条</span>
    </div>
    <p v-if="visitsError" class="error-message" role="alert">
      {{ visitsError }}
      <button class="text-btn" @click="loadVisits">重试</button>
    </p>
    <div v-if="visitsLoading" class="mini-empty">正在加载…</div>
    <div v-else-if="visits" class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>访问时间</th>
            <th>IP</th>
            <th>来源</th>
            <th>设备 / 浏览器</th>
            <th>类型</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(v, i) in visits.visits" :key="i">
            <td>{{ formatDate(v.visited_at) }}</td>
            <td>{{ v.ip || "未知" }}</td>
            <td>{{ v.referrer_domain || "直接访问 / 未知" }}</td>
            <td>
              {{
                {
                  mobile: "手机",
                  desktop: "桌面设备",
                  tablet: "平板",
                  unknown: "未知",
                }[v.device] || v.device
              }}
              / {{ v.browser === "unknown" ? "未知" : v.browser }}
            </td>
            <td>
              <span class="tiny-badge">{{
                v.is_bot ? "疑似机器人" : "普通访问"
              }}</span>
            </td>
          </tr>
          <tr v-if="!visits.visits.length">
            <td colspan="5" class="muted">暂无访问记录</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div v-if="visits && visits.total_visits > 50" class="pagination">
      <span
        >第 {{ visitPage }} / {{ Math.ceil(visits.total_visits / 50) }} 页</span
      >
      <div>
        <button
          class="btn"
          :disabled="visitPage === 1 || visitsLoading"
          @click="visitPage--"
        >
          上一页</button
        ><button
          class="btn"
          :disabled="visitPage * 50 >= visits.total_visits || visitsLoading"
          @click="visitPage++"
        >
          下一页
        </button>
      </div>
    </div>
  </section>
</template>
