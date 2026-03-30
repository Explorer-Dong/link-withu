<template>
  <div class="home">
    <section class="create-card">
      <h2>生成短链</h2>
      <form @submit.prevent="onSubmit" class="form">
        <div class="field">
          <label for="url">原始链接</label>
          <input
            id="url"
            v-model="form.originalUrl"
            type="url"
            placeholder="https://example.com/very-long-url"
            required
          />
        </div>
        <div class="field">
          <label for="code">自定义短码（选填，不填则随机生成）</label>
          <input
            id="code"
            v-model="form.shortCode"
            type="text"
            placeholder="my-link"
            maxlength="32"
          />
        </div>
        <button type="submit" class="btn primary" :disabled="loading">
          {{ loading ? '生成中…' : '生成短链' }}
        </button>
        <p v-if="error" class="error">{{ error }}</p>
        <div v-if="created" class="result">
          <p class="result-label">短链已生成：</p>
          <div class="result-row">
            <input readonly :value="created.short_url" class="result-input" ref="resultInput" />
            <button type="button" class="btn secondary" @click="copyShortUrl">复制</button>
          </div>
        </div>
      </form>
    </section>

    <section class="list-card">
      <div class="list-head">
        <h2>短链列表</h2>
        <button type="button" class="btn secondary" @click="loadList" :disabled="listLoading">刷新</button>
      </div>
      <div v-if="listLoading" class="loading">加载中…</div>
      <div v-else-if="!list.items.length" class="empty">暂无短链，请先生成</div>
      <ul v-else class="link-list">
        <li v-for="item in list.items" :key="item.id" class="link-item">
          <div class="link-main">
            <span class="link-code">/r/{{ item.short_code }}</span>
            <span class="link-url">{{ item.original_url }}</span>
            <span class="link-meta">
              访问 {{ item.visit_count }} 次 · {{ formatDate(item.created_at) }}
            </span>
          </div>
          <div class="link-actions">
            <button type="button" class="btn small" @click="copyItem(item)">复制</button>
            <button type="button" class="btn small" @click="openStats(item)">统计</button>
          </div>
        </li>
      </ul>
    </section>

    <!-- 统计弹窗 -->
    <div v-if="statsModal" class="modal-overlay" @click.self="statsModal = null">
      <div class="modal">
        <div class="modal-head">
          <h3>访问统计 · {{ statsModal.short_code }}</h3>
          <button type="button" class="modal-close" @click="statsModal = null">×</button>
        </div>
        <div class="modal-body">
          <p class="stats-original">{{ statsModal.original_url }}</p>
          <p class="stats-total">总访问：<strong>{{ statsModal.total_visits }}</strong> 次</p>
          <div class="visits-table-wrap">
            <table class="visits-table">
              <thead>
                <tr>
                  <th>时间</th>
                  <th>IP</th>
                  <th>User-Agent</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(v, i) in statsModal.visits" :key="i">
                  <td>{{ v.visited_at }}</td>
                  <td>{{ v.ip || '-' }}</td>
                  <td class="ua">{{ v.user_agent || '-' }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { createLink, listLinks, getStats } from '../api/links'

const form = ref({ originalUrl: '', shortCode: '' })
const loading = ref(false)
const error = ref('')
const created = ref(null)
const resultInput = ref(null)

const list = ref({ total: 0, items: [] })
const listLoading = ref(false)
const statsModal = ref(null)

async function onSubmit() {
  error.value = ''
  created.value = null
  loading.value = true
  try {
    const payload = {
      original_url: form.value.originalUrl,
    }
    if (form.value.shortCode?.trim()) payload.short_code = form.value.shortCode.trim()
    const data = await createLink(payload.original_url, payload.short_code || null)
    created.value = data
    form.value.originalUrl = ''
    form.value.shortCode = ''
    loadList()
  } catch (e) {
    error.value = e.message || '创建失败'
  } finally {
    loading.value = false
  }
}

function copyShortUrl() {
  if (!created.value?.short_url || !resultInput.value) return
  navigator.clipboard.writeText(created.value.short_url).then(() => {
    const btn = resultInput.value?.nextElementSibling
    if (btn) { btn.textContent = '已复制'; setTimeout(() => { btn.textContent = '复制' }, 1500) }
  })
}

function copyItem(item) {
  navigator.clipboard.writeText(item.short_url).then(() => {})
}

async function loadList() {
  listLoading.value = true
  try {
    list.value = await listLinks(0, 50)
  } catch {
    list.value = { total: 0, items: [] }
  } finally {
    listLoading.value = false
  }
}

async function openStats(item) {
  try {
    const data = await getStats(item.short_code)
    statsModal.value = data
  } catch {
    statsModal.value = null
  }
}

function formatDate(iso) {
  if (!iso) return '-'
  const d = new Date(iso)
  return d.toLocaleString('zh-CN')
}

onMounted(() => loadList())
</script>

<style scoped>
  .create-card, .list-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 1.5rem;
    margin-bottom: 1.5rem;
  }
  .create-card h2, .list-card h2 { margin: 0 0 1rem; font-size: 1.1rem; color: var(--text); }
  .form .field { margin-bottom: 1rem; }
  .form label { display: block; margin-bottom: 0.35rem; color: var(--muted); font-size: 0.9rem; }
  .form input[type="url"], .form input[type="text"] {
    width: 100%; padding: 0.6rem 0.75rem; background: var(--bg); border: 1px solid var(--border);
    border-radius: 8px; color: var(--text); font-size: 1rem;
  }
  .form input:focus { outline: none; border-color: var(--accent); }
  .btn {
    padding: 0.5rem 1rem; border-radius: 8px; border: none; cursor: pointer; font-size: 0.95rem;
  }
  .btn.primary { background: var(--accent); color: #fff; }
  .btn.primary:hover:not(:disabled) { background: var(--accent-hover); }
  .btn.primary:disabled { opacity: 0.6; cursor: not-allowed; }
  .btn.secondary { background: var(--border); color: var(--text); }
  .btn.small { padding: 0.35rem 0.65rem; font-size: 0.85rem; }
  .error { color: var(--danger); margin-top: 0.5rem; font-size: 0.9rem; }
  .result { margin-top: 1rem; padding-top: 1rem; border-top: 1px solid var(--border); }
  .result-label { margin: 0 0 0.5rem; color: var(--muted); font-size: 0.9rem; }
  .result-row { display: flex; gap: 0.5rem; align-items: center; }
  .result-input { flex: 1; padding: 0.5rem; background: var(--bg); border: 1px solid var(--border); border-radius: 6px; color: var(--success); font-size: 0.9rem; }
  .list-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
  .link-list { list-style: none; margin: 0; padding: 0; }
  .link-item {
    display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem;
    padding: 0.75rem 0; border-bottom: 1px solid var(--border);
  }
  .link-item:last-child { border-bottom: none; }
  .link-main { flex: 1; min-width: 0; }
  .link-code { display: inline-block; color: var(--accent); font-family: monospace; margin-right: 0.5rem; }
  .link-url { color: var(--muted); font-size: 0.9rem; word-break: break-all; }
  .link-meta { display: block; margin-top: 0.25rem; color: var(--muted); font-size: 0.8rem; }
  .link-actions { display: flex; gap: 0.5rem; flex-shrink: 0; }
  .loading, .empty { color: var(--muted); padding: 1rem 0; text-align: center; }
  .modal-overlay {
    position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center;
    z-index: 100; padding: 1rem;
  }
  .modal { background: var(--surface); border: 1px solid var(--border); border-radius: 12px; max-width: 640px; width: 100%; max-height: 85vh; display: flex; flex-direction: column; }
  .modal-head { display: flex; justify-content: space-between; align-items: center; padding: 1rem 1.25rem; border-bottom: 1px solid var(--border); }
  .modal-head h3 { margin: 0; font-size: 1rem; }
  .modal-close { background: none; border: none; color: var(--muted); font-size: 1.5rem; cursor: pointer; line-height: 1; }
  .modal-body { padding: 1.25rem; overflow: auto; }
  .stats-original { color: var(--muted); font-size: 0.9rem; word-break: break-all; margin: 0 0 0.5rem; }
  .stats-total { margin: 0 0 1rem; }
  .visits-table-wrap { overflow-x: auto; }
  .visits-table { width: 100%; border-collapse: collapse; font-size: 0.85rem; }
  .visits-table th, .visits-table td { padding: 0.5rem 0.75rem; text-align: left; border-bottom: 1px solid var(--border); }
  .visits-table th { color: var(--muted); font-weight: 500; }
  .visits-table .ua { max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
