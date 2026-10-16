<script setup>
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import Icon from "../components/Icon.vue";
import Modal from "../components/Modal.vue";
import { createLink, listLinks, updateLink, deleteLink } from "../api/links";
import { copy, formatDate, notify, number, tags } from "../state";
const route = useRoute(),
  router = useRouter();
const items = ref([]),
  total = ref(0),
  page = ref(1),
  q = ref(""),
  sort = ref("newest"),
  loading = ref(false),
  error = ref(""),
  saving = ref(false);
const emptyForm = () => ({
  original_url: "",
  short_code: "",
  title: "",
  note: "",
  tagText: "",
});
const form = reactive(emptyForm()),
  edit = ref(null),
  removing = ref(null),
  created = ref(null),
  expanded = ref(false);
const activeTag = computed(() => route.query.tag || "");
const pageSize = 10;
const pages = computed(() => Math.max(1, Math.ceil(total.value / pageSize)));
let timer,
  generation = 0;
function body(value) {
  return {
    original_url: value.original_url,
    title: value.title,
    note: value.note,
    tags: value.tagText
      .split(/[,，]/)
      .map((t) => t.trim())
      .filter(Boolean),
  };
}
async function load() {
  const current = ++generation;
  loading.value = true;
  error.value = "";
  try {
    const data = await listLinks({
      skip: (page.value - 1) * pageSize,
      limit: pageSize,
      q: q.value,
      sort: sort.value,
      tag: activeTag.value,
    });
    if (current !== generation) return;
    items.value = data.items;
    total.value = data.total;
    if (page.value > pages.value) {
      page.value = pages.value;
      return load();
    }
  } catch (e) {
    if (current === generation) error.value = e.message;
  } finally {
    if (current === generation) loading.value = false;
  }
}
function changed() {
  window.dispatchEvent(new Event("links-changed"));
  load();
}
async function create() {
  saving.value = true;
  try {
    created.value = await createLink({
      ...body(form),
      short_code: form.short_code || null,
    });
    Object.assign(form, emptyForm());
    expanded.value = false;
    page.value = 1;
    changed();
    notify("短链已创建");
  } catch (e) {
    notify(e.message, true);
  } finally {
    saving.value = false;
  }
}
async function saveEdit() {
  saving.value = true;
  try {
    await updateLink(edit.value.short_code, body(edit.value));
    edit.value = null;
    changed();
    notify("修改已保存");
  } catch (e) {
    notify(e.message, true);
  } finally {
    saving.value = false;
  }
}
async function remove() {
  saving.value = true;
  try {
    await deleteLink(removing.value.short_code);
    removing.value = null;
    changed();
    notify("短链已删除");
  } catch (e) {
    notify(e.message, true);
  } finally {
    saving.value = false;
  }
}
watch(q, () => {
  clearTimeout(timer);
  timer = setTimeout(() => {
    page.value = 1;
    load();
  }, 300);
});
watch([sort, activeTag], () => {
  page.value = 1;
  load();
});
onMounted(load);
onUnmounted(() => {
  clearTimeout(timer);
  generation++;
});
</script>
<template>
  <div class="page-heading">
    <div><h1>链接管理</h1></div>
  </div>
  <section class="card composer">
    <div class="section-heading">
      <h2><Icon name="plus" :size="18" />创建短链</h2>
      <button type="button" class="text-btn" @click="expanded = !expanded">
        {{ expanded ? "收起选项" : "更多选项"
        }}<Icon :name="expanded ? 'chevron-up' : 'chevron-down'" :size="16" />
      </button>
    </div>
    <form @submit.prevent="create">
      <div class="url-create">
        <label class="sr-only" for="new-url">目标链接</label>
        <div class="input-with-icon">
          <Icon name="link" :size="18" /><input
            id="new-url"
            v-model="form.original_url"
            placeholder="粘贴你的长链接，例如 https://example.com/article"
            required
            maxlength="2048"
          />
        </div>
        <button class="btn primary" :disabled="saving">
          <Icon name="plus" :size="17" />{{ saving ? "创建中…" : "创建短链" }}
        </button>
      </div>
      <div v-if="expanded" class="form-grid optional-fields">
        <label
          >链接名称<input
            v-model="form.title"
            placeholder="给链接起个名字"
            maxlength="120" /></label
        ><label
          >自定义短码<input
            v-model="form.short_code"
            placeholder="留空自动生成"
            pattern="[A-Za-z0-9_-]{1,32}"
            maxlength="32" /></label
        ><label
          >标签<input
            v-model="form.tagText"
            placeholder="博客, 公众号, 活动" /></label
        ><label
          >备注<input
            v-model="form.note"
            placeholder="记录链接的用途"
            maxlength="2000"
        /></label>
      </div>
    </form>
    <div v-if="created" class="created-result">
      <span
        ><span class="status-dot"></span>已创建
        <a :href="created.short_url" target="_blank" rel="noreferrer">{{
          created.short_url
        }}</a></span
      ><button class="text-btn" @click="copy(created.short_url)">
        <Icon name="copy" :size="15" />复制链接
      </button>
    </div>
  </section>
  <section class="links-section">
    <div class="list-toolbar">
      <div class="toolbar-title">
        <h2>{{ activeTag ? "# " + activeTag : "全部链接" }}</h2>
        <button
          v-if="activeTag"
          class="text-btn muted"
          @click="router.push('/links')"
        >
          清除筛选 <Icon name="close" :size="16" />
        </button>
      </div>
      <div class="filters">
        <div class="input-with-icon search-field">
          <Icon name="search" :size="17" /><input
            v-model="q"
            aria-label="搜索链接"
            placeholder="搜索名称、链接或标签…"
            maxlength="200"
          />
        </div>
        <select
          :value="activeTag"
          aria-label="筛选标签"
          @change="
            router.push({
              path: '/links',
              query: { tag: $event.target.value || undefined },
            })
          "
        >
          <option value="">全部标签</option>
          <option v-for="tag in tags" :key="tag.name" :value="tag.name">
            # {{ tag.name }}
          </option>
        </select>
        <select v-model="sort" aria-label="排序方式">
          <option value="newest">最新创建</option>
          <option value="oldest">最早创建</option>
          <option value="visits">访问最多</option>
        </select>
      </div>
    </div>
    <p v-if="error" class="error-message" role="alert">
      {{ error }} <button class="text-btn" @click="load">重试</button>
    </p>
    <div v-if="loading" class="loading-state" aria-live="polite">
      正在加载链接…
    </div>
    <div v-else-if="!items.length && !error" class="empty-state card">
      <span class="empty-icon"><Icon name="link" :size="28" /></span>
      <h3>
        {{ q || activeTag ? "没有找到匹配的链接" : "从你的第一个链接开始" }}
      </h3>
    </div>
    <div v-else class="link-list">
      <article v-for="item in items" :key="item.id" class="card link-card">
        <div class="link-card-icon"><Icon name="link" :size="22" /></div>
        <div class="link-content">
          <div class="link-title-row">
            <h3>{{ item.title || item.short_code }}</h3>
            <span v-if="item.is_custom" class="tiny-badge">自定义</span>
          </div>
          <a
            class="short-link"
            :href="item.short_url"
            target="_blank"
            rel="noreferrer"
            >{{ item.short_url }}<Icon name="arrow" :size="13"
          /></a>
          <p class="original-url" :title="item.original_url">
            {{ item.original_url }}
          </p>
          <p v-if="item.note" class="link-note">{{ item.note }}</p>
          <div class="link-meta">
            <button
              v-for="tag in item.tags"
              :key="tag"
              class="tag-pill"
              @click="router.push({ path: '/links', query: { tag } })"
            >
              # {{ tag }}</button
            ><span>{{ formatDate(item.created_at) }}</span>
          </div>
        </div>
        <div class="link-right">
          <router-link
            :aria-label="`访问统计：${item.short_code}`"
            class="visit-pill"
            :to="{ path: '/', query: { short_code: item.short_code } }"
            ><Icon name="chart" :size="16" /><strong>{{
              number(item.visit_count)
            }}</strong
            ><span>次访问</span></router-link
          >
          <div class="link-actions">
            <button
              class="icon-btn"
              aria-label="复制短链"
              title="复制短链"
              @click="copy(item.short_url)"
            >
              <Icon name="copy" :size="17" /></button
            ><button
              class="icon-btn"
              aria-label="编辑短链"
              title="编辑短链"
              @click="edit = { ...item, tagText: item.tags.join(', ') }"
            >
              <Icon name="edit" :size="17" /></button
            ><button
              class="icon-btn danger-text"
              aria-label="删除短链"
              title="删除短链"
              @click="removing = item"
            >
              <Icon name="trash" :size="17" />
            </button>
          </div>
        </div>
      </article>
    </div>
    <div v-if="total > pageSize" class="pagination" aria-label="链接分页">
      <span>共 {{ number(total) }} 条 · 第 {{ page }} / {{ pages }} 页</span>
      <div>
        <button
          class="btn"
          :disabled="page === 1 || loading"
          @click="
            page--;
            load();
          "
        >
          上一页</button
        ><button
          class="btn"
          :disabled="page >= pages || loading"
          @click="
            page++;
            load();
          "
        >
          下一页
        </button>
      </div>
    </div>
  </section>
  <Modal v-if="edit" title="编辑短链" @close="!saving && (edit = null)"
    ><form class="stack-form" @submit.prevent="saveEdit">
      <p class="muted small">
        短码 {{ edit.short_code }} 保持不变，已有分享继续有效。
      </p>
      <label
        >目标链接<input
          v-model="edit.original_url"
          required
          maxlength="2048" /></label
      ><label>链接名称<input v-model="edit.title" maxlength="120" /></label
      ><label
        >标签<input
          v-model="edit.tagText"
          placeholder="多个标签用逗号分隔" /></label
      ><label
        >备注<textarea v-model="edit.note" rows="3" maxlength="2000"></textarea>
      </label>
      <div class="modal-actions">
        <button
          type="button"
          class="btn"
          :disabled="saving"
          @click="edit = null"
        >
          取消</button
        ><button class="btn primary" :disabled="saving">
          {{ saving ? "保存中…" : "保存修改" }}
        </button>
      </div>
    </form></Modal
  >
  <Modal
    v-if="removing"
    title="删除这个短链？"
    @close="!saving && (removing = null)"
    ><p class="muted">
      {{ removing.title || removing.short_code }}
      的访问记录也会被删除，此操作无法撤销。
    </p>
    <div class="modal-actions">
      <button class="btn" :disabled="saving" @click="removing = null">
        取消</button
      ><button class="btn danger" :disabled="saving" @click="remove">
        {{ saving ? "删除中…" : "确认删除" }}
      </button>
    </div></Modal
  >
</template>
