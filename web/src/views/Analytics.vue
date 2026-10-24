<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import PageHeading from "../components/PageHeading.vue";
import Icon from "../components/Icon.vue";
import TrendChart from "../components/TrendChart.vue";
import Breakdown from "../components/Breakdown.vue";
import AnalyticsDetails from "../components/AnalyticsDetails.vue";
import { api, getStats, refreshTags } from "../api/links";
import { tags, number, formatDate } from "../state";
const route = useRoute(),
  router = useRouter();
const selectedRange = ref({}),
  draftStart = ref(""),
  draftEnd = ref(""),
  rangeError = ref(""),
  rangePicker = ref(null),
  details = ref(null),
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
  visitGeneration = 0,
  locationTimer;
const filters = computed(() => ({
  ...selectedRange.value,
  exclude_bots: excludeBots.value,
  short_code: code.value,
  tag: tag.value,
}));
const rangeLabel = computed(() =>
  data.value ? `${data.value.range_start} 至 ${data.value.range_end}` : "",
);
function applyRange() {
  if (draftStart.value > draftEnd.value) {
    rangeError.value = "开始日期不能晚于结束日期";
    return;
  }
  selectedRange.value = {
    start_date: draftStart.value,
    end_date: draftEnd.value,
  };
  rangeError.value = "";
  rangePicker.value.open = false;
}
function resetRange() {
  selectedRange.value = {};
  draftStart.value = draftEnd.value = rangeError.value = "";
  rangePicker.value.open = false;
}
function selectLink(value) {
  details.value = null;
  router.push({ path: "/", query: { ...route.query, short_code: value } });
}
const tagOptions = computed(() => {
  const available = [...tags.value];
  if (tag.value && !available.some((item) => item.name === tag.value))
    available.push({ name: tag.value });
  return available;
});
const metrics = computed(() => {
  if (!data.value) return [];
  const { summary, range_start, range_end } = data.value;
  const countDays = Math.max(
    1,
    Math.round((Date.parse(range_end) - Date.parse(range_start)) / 86400000) +
      1,
  );
  return [
    {
      title: "访问量",
      value: summary.period_visits,
      icon: "eye",
      foot: "所选时间区间",
    },
    {
      title: "日均访问",
      value: Math.round((summary.period_visits / countDays) * 10) / 10,
      icon: "chart",
      foot: `共 ${number(countDays)} 天`,
    },
    {
      title: "疑似机器人",
      value: summary.suspected_bots,
      icon: "clock",
      foot: excludeBots.value ? "已从访问量中排除" : "包含在访问量中",
    },
    {
      title: "活跃短链",
      value: summary.active_links,
      icon: "link",
      foot: `共 ${number(summary.links)} 个链接`,
    },
  ];
});
async function load(silent = false) {
  clearTimeout(locationTimer);
  const current = ++generation;
  if (silent !== true) loading.value = true;
  error.value = "";
  try {
    const [result] = await Promise.all([
      api(`/api/analytics?${new URLSearchParams(filters.value)}`),
      refreshTags(),
    ]);
    if (current === generation) {
      if (
        silent === true &&
        data.value &&
        JSON.stringify(result.trend) === JSON.stringify(data.value.trend)
      )
        result.trend = data.value.trend;
      data.value = result;
      if (result.summary.pending_locations > 0) {
        locationTimer = setTimeout(() => {
          load(true);
          loadVisits(true);
        }, 3000);
      }
    }
  } catch (e) {
    if (current === generation) error.value = e.message;
  } finally {
    if (current === generation) loading.value = false;
  }
}
async function loadVisits(silent = false) {
  const current = ++visitGeneration;
  if (!code.value) {
    visits.value = null;
    return;
  }
  if (silent !== true) visitsLoading.value = true;
  visitsError.value = "";
  try {
    const result = await getStats(code.value, {
      ...selectedRange.value,
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
watch([code, excludeBots, selectedRange], () => {
  visitPage.value = 1;
  loadVisits();
});
watch(visitPage, loadVisits);
onMounted(() => {
  load();
  loadVisits();
});
onUnmounted(() => {
  clearTimeout(locationTimer);
  generation++;
  visitGeneration++;
});
</script>
<template>
  <PageHeading title="首页" />
  <div class="analytics-toolbar">
    <div class="filters">
      <details ref="rangePicker" class="date-range-picker">
        <summary>
          <Icon name="calendar" :size="17" />{{
            selectedRange.start_date
              ? `${selectedRange.start_date} 至 ${selectedRange.end_date}`
              : "选择时间区间"
          }}<Icon name="chevron-down" :size="15" />
        </summary>
        <form class="date-range-panel card" @submit.prevent="applyRange">
          <div class="date-range-fields">
            <label
              >开始日期<input
                v-model="draftStart"
                aria-label="开始日期"
                type="date"
                min="0001-01-01"
                max="9998-12-31"
                required
            /></label>
            <label
              >结束日期<input
                v-model="draftEnd"
                aria-label="结束日期"
                type="date"
                min="0001-01-01"
                max="9998-12-31"
                required
            /></label>
          </div>
          <p v-if="rangeError" class="error-message" role="alert">
            {{ rangeError }}
          </p>
          <div class="date-range-actions">
            <button class="text-btn" type="button" @click="resetRange">
              重置</button
            ><button class="btn primary" type="submit">应用</button>
          </div>
        </form>
      </details>
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
          <h2>访问量趋势</h2>
        </div>
        <span class="chart-legend"
          ><span class="status-dot"></span>{{ rangeLabel }} · 共
          {{ number(data.summary.period_visits) }} 次访问</span
        >
      </div>
      <TrendChart :points="data.trend" :label="rangeLabel" />
      <p class="muted small chart-footnote">
        <span
          >{{ data.timezone
          }}<template v-if="data.bucket_days > 1">
            · 每 {{ data.bucket_days }} 天汇总</template
          ></span
        >
        <span>
          疑似机器人：{{ number(data.summary.suspected_bots) }} 次{{
            excludeBots ? "（已排除）" : ""
          }}</span
        >
      </p>
    </section>
    <div class="breakdowns-grid">
      <Breakdown
        title="访问来源"
        @open="details = { kind: 'referrers', title: '访问来源' }"
        :items="data.referrers"
        :total="data.summary.period_visits"
      /><Breakdown
        title="设备分布"
        @open="details = { kind: 'devices', title: '设备分布' }"
        :items="data.devices"
        :total="data.summary.period_visits"
      /><Breakdown
        title="浏览器"
        @open="details = { kind: 'browsers', title: '浏览器' }"
        :items="data.browsers"
        :total="data.summary.period_visits"
      /><Breakdown
        title="地理位置"
        @open="details = { kind: 'locations', title: '地理位置' }"
        :items="data.locations"
        :total="data.summary.period_visits"
      />
    </div>
    <div class="analytics-bottom-grid">
      <section class="card ranking">
        <div class="section-heading">
          <h2>
            <button
              class="section-title-button"
              aria-label="热门短链"
              @click="details = { kind: 'top_links', title: '热门短链' }"
            >
              热门短链
            </button>
          </h2>
          <span class="muted small">{{ rangeLabel }}</span>
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
          <h2>
            <button
              class="section-title-button"
              aria-label="标签分组"
              @click="details = { kind: 'tags', title: '标签分组' }"
            >
              标签分组
            </button>
          </h2>
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
    <AnalyticsDetails
      v-if="details"
      :kind="details.kind"
      :title="details.title"
      :filters="filters"
      :revision="data.summary.pending_locations"
      @close="details = null"
      @tag="
        (value) => {
          details = null;
          setTag(value);
        }
      "
      @link="selectLink"
    />
  </template>
  <section v-if="code" class="card visit-details">
    <div class="section-heading">
      <div>
        <h2>访问明细</h2>
        <p class="muted small">
          {{ selectedRange.start_date ? rangeLabel : "全部历史" }}
        </p>
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
            <th>地址</th>
            <th>来源</th>
            <th>设备 / 浏览器</th>
            <th>类型</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(v, i) in visits.visits" :key="i">
            <td>{{ formatDate(v.visited_at) }}</td>
            <td>{{ v.ip || "未知" }}</td>
            <td>{{ v.location || "未知" }}</td>
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
            <td colspan="6" class="muted">暂无访问记录</td>
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
