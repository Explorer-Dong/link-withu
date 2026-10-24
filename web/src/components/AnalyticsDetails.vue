<script setup>
import { computed, onUnmounted, ref, watch } from "vue";
import Modal from "./Modal.vue";
import { api } from "../api/links";
import { number } from "../state";
const props = defineProps({
  kind: String,
  title: String,
  filters: Object,
  revision: Number,
});
const emit = defineEmits(["close", "tag", "link"]);
const page = ref(1),
  data = ref(null),
  loading = ref(false),
  error = ref("");
const pageSize = 6;
const pages = computed(() =>
  Math.max(1, Math.ceil((data.value?.total || 0) / pageSize)),
);
const names = {
  unknown: "未知",
  desktop: "桌面设备",
  mobile: "手机",
  tablet: "平板",
};
let generation = 0;
async function load() {
  const current = ++generation;
  loading.value = true;
  error.value = "";
  try {
    const result = await api(
      `/api/analytics/breakdown?${new URLSearchParams({ ...props.filters, kind: props.kind, skip: (page.value - 1) * pageSize, limit: pageSize })}`,
    );
    if (current === generation) {
      const lastPage = Math.max(1, Math.ceil(result.total / pageSize));
      if (page.value > lastPage) page.value = lastPage;
      else data.value = result;
    }
  } catch (e) {
    if (current === generation) error.value = e.message;
  } finally {
    if (current === generation) loading.value = false;
  }
}
watch(
  [() => props.kind, () => props.filters],
  () => {
    if (page.value !== 1) page.value = 1;
    else load();
  },
  { immediate: true },
);
watch(page, load);
watch(
  () => props.revision,
  () => {
    if (props.kind === "locations") load();
  },
);
onUnmounted(() => generation++);
</script>
<template>
  <Modal :title="title" @close="emit('close')">
    <p v-if="error" class="error-message" role="alert">
      {{ error }} <button class="text-btn" @click="load">重试</button>
    </p>
    <div v-if="loading" class="mini-empty" aria-live="polite">正在加载…</div>
    <template v-else-if="data && !error">
      <div v-if="!data.items.length" class="mini-empty">暂无访问数据</div>
      <div
        v-for="(item, index) in data.items"
        :key="item.short_code || item.name"
        class="detail-row"
      >
        <span class="rank-number">{{
          String((page - 1) * pageSize + index + 1).padStart(2, "0")
        }}</span>
        <button
          v-if="kind === 'top_links'"
          class="detail-label"
          @click="emit('link', item.short_code)"
        >
          <strong>{{ item.title || item.short_code }}</strong
          ><small>{{ item.original_url }}</small>
        </button>
        <button
          v-else-if="kind === 'tags'"
          class="detail-label"
          @click="emit('tag', item.name)"
        >
          <strong># {{ item.name }}</strong
          ><small>{{ number(item.link_count) }} 个链接</small>
        </button>
        <span v-else class="detail-label">{{
          names[item.name] || item.name
        }}</span>
        <strong class="detail-count"
          >{{ number(item.count) }}<small> 次</small></strong
        >
      </div>
      <div class="pagination detail-pagination">
        <span
          >共 {{ number(data.total) }} 项 · 第 {{ page }} / {{ pages }} 页</span
        >
        <div>
          <button class="btn" :disabled="page === 1" @click="page--">
            上一页</button
          ><button class="btn" :disabled="page >= pages" @click="page++">
            下一页
          </button>
        </div>
      </div>
    </template>
  </Modal>
</template>
