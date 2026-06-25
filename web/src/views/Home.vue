<template>
  <div class="home">
    <section class="create-card">
      <div class="section-title">
        <p class="eyebrow">Create</p>
        <h2>生成短链</h2>
      </div>
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
          <p class="result-label">短链已生成</p>
          <div class="result-row">
            <input readonly :value="created.short_url" class="result-input" ref="resultInput" />
            <button type="button" class="btn secondary" @click="copyShortUrl">复制</button>
          </div>
        </div>
      </form>
    </section>

    <section class="list-card">
      <div class="list-head">
        <div class="section-title">
          <p class="eyebrow">Links</p>
          <h2>短链列表</h2>
        </div>
        <div class="list-head-right">
          <button type="button" class="btn secondary" @click="onAdminClick">
            {{ adminSession ? '退出管理员' : '管理员登录' }}
          </button>
          <input
            v-model="searchInput"
            type="text"
            class="search-input"
            placeholder="搜索链接…"
            @input="onSearchInput"
          />
          <button type="button" class="btn secondary" @click="loadList" :disabled="listLoading">刷新</button>
        </div>
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
            <button type="button" class="btn small danger-btn" @click="onDelete(item)">删除</button>
          </div>
        </li>
      </ul>

      <div v-if="list.total > pageSize" class="pagination">
        <button class="btn small" :disabled="page <= 1" @click="goPage(page - 1)">上一页</button>
        <span class="page-info">{{ page }} / {{ totalPages }}</span>
        <button class="btn small" :disabled="page >= totalPages" @click="goPage(page + 1)">下一页</button>
      </div>
    </section>

    <Transition name="toast">
      <div v-if="toast" class="toast">{{ toast }}</div>
    </Transition>

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
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { createLink, listLinks, deleteLink, getStats, adminLogin } from '../api/links'

const form = ref({ originalUrl: '', shortCode: '' })
const loading = ref(false)
const error = ref('')
const created = ref(null)
const resultInput = ref(null)

const list = ref({ total: 0, items: [] })
const listLoading = ref(false)
const statsModal = ref(null)

const searchInput = ref('')
const searchQuery = ref('')
const pageSize = 20
const page = ref(1)
const toast = ref('')
let toastTimer = null
let debounceTimer = null
const ADMIN_SESSION_KEY = 'link-withu-admin-session'

const totalPages = computed(() => Math.max(1, Math.ceil(list.value.total / pageSize)))
const adminSession = ref(loadAdminSession())

function loadAdminSession() {
  const raw = localStorage.getItem(ADMIN_SESSION_KEY)
  if (!raw) return null

  try {
    const session = JSON.parse(raw)
    if (!session.token || session.expiresAt <= Math.floor(Date.now() / 1000)) {
      localStorage.removeItem(ADMIN_SESSION_KEY)
      return null
    }
    return session
  } catch {
    localStorage.removeItem(ADMIN_SESSION_KEY)
    return null
  }
}

function saveAdminSession(data) {
  const session = { token: data.token, expiresAt: data.expires_at }
  localStorage.setItem(ADMIN_SESSION_KEY, JSON.stringify(session))
  adminSession.value = session
}

function clearAdminSession() {
  localStorage.removeItem(ADMIN_SESSION_KEY)
  adminSession.value = null
}

async function loginAdmin() {
  const password = prompt('请输入管理员密码：')
  if (password === null) return null
  const data = await adminLogin(password)
  saveAdminSession(data)
  showToast('管理员登录成功，24 小时内无需重复输入密码')
  return data.token
}

async function ensureAdminToken() {
  adminSession.value = loadAdminSession()
  if (adminSession.value) return adminSession.value.token
  return loginAdmin()
}

async function onAdminClick() {
  if (adminSession.value) {
    clearAdminSession()
    showToast('已退出管理员登录')
    return
  }

  try {
    await loginAdmin()
  } catch (e) {
    showToast(e.message || '登录失败')
  }
}

function showToast(msg) {
  toast.value = msg
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => { toast.value = '' }, 2000)
}

function onSearchInput() {
  clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => {
    searchQuery.value = searchInput.value.trim()
    page.value = 1
    loadList()
  }, 350)
}

function goPage(p) {
  page.value = p
  loadList()
}

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
    showToast('已复制到剪贴板')
  })
}

function copyItem(item) {
  navigator.clipboard.writeText(item.short_url).then(() => {
    showToast('已复制到剪贴板')
  })
}

async function onDelete(item) {
  if (!confirm(`确定删除短链 /r/${item.short_code} ？`)) return
  try {
    const token = await ensureAdminToken()
    if (!token) return
    await deleteLink(item.short_code, token)
    showToast('已删除')
    if (list.value.items.length === 1 && page.value > 1) page.value -= 1
    loadList()
  } catch (e) {
    if (e.message?.includes('过期') || e.message?.includes('无效') || e.message?.includes('登录')) clearAdminSession()
    showToast(e.message || '删除失败')
  }
}

async function loadList() {
  listLoading.value = true
  try {
    const skip = (page.value - 1) * pageSize
    list.value = await listLinks(skip, pageSize, searchQuery.value)
  } catch {
    list.value = { total: 0, items: [] }
  } finally {
    listLoading.value = false
  }
}

async function openStats(item) {
  try {
    const token = await ensureAdminToken()
    if (!token) return
    statsModal.value = await getStats(item.short_code, token)
  } catch (e) {
    if (e.message?.includes('过期') || e.message?.includes('无效') || e.message?.includes('登录')) clearAdminSession()
    showToast(e.message || '获取统计失败')
    statsModal.value = null
  }
}

function formatDate(iso) {
  if (!iso) return '-'
  const d = new Date(iso)
  return d.toLocaleString('zh-CN')
}

onMounted(() => loadList())
onUnmounted(() => {
  clearTimeout(debounceTimer)
  clearTimeout(toastTimer)
})
</script>

<style scoped>
  .home {
    display: grid;
    grid-template-columns: minmax(280px, 360px) minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
  }
  .create-card, .list-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: var(--shadow);
  }
  .create-card { padding: 1.2rem; }
  .list-card { padding: 1.2rem; }
  .section-title { margin-bottom: 1rem; }
  .eyebrow {
    margin: 0 0 0.25rem;
    color: var(--accent);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .create-card h2, .list-card h2 { margin: 0; font-size: 1.05rem; color: var(--text); }
  .form .field { margin-bottom: 1rem; }
  .form label { display: block; margin-bottom: 0.4rem; color: var(--text); font-size: 0.86rem; font-weight: 600; }
  .form input[type="url"], .form input[type="text"] {
    width: 100%;
    padding: 0.62rem 0.72rem;
    background: var(--surface-soft);
    border: 1px solid var(--border);
    border-radius: 7px;
    color: var(--text);
    font-size: 0.95rem;
    height: 38px;
  }
  .form input:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
  .btn {
    padding: 0.5rem 0.85rem;
    border-radius: 7px;
    border: 1px solid transparent;
    cursor: pointer;
    font-size: 0.9rem;
    font-weight: 600;
    transition: background 0.15s ease, border-color 0.15s ease, color 0.15s ease;
    height: 38px; /* 和 input 的默认高度保持一致 */
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .btn.primary { background: var(--accent); color: #1b120b; border-color: var(--accent); }
  .btn.primary:hover:not(:disabled) { background: var(--accent-hover); border-color: var(--accent-hover); }
  .btn.primary:disabled { opacity: 0.6; cursor: not-allowed; }
  .btn.secondary, .btn.small { background: var(--control-bg); color: var(--text); border-color: var(--border); }
  .btn.secondary:hover:not(:disabled), .btn.small:hover:not(:disabled) { background: var(--control-hover); border-color: var(--accent); }
  .btn.small { padding: 0.36rem 0.62rem; font-size: 0.82rem; height: 38px; }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .danger-btn { background: var(--danger-bg); color: var(--danger); }
  .danger-btn:hover:not(:disabled) { background: var(--danger-hover); border-color: var(--danger); color: var(--danger); }
  .error { color: var(--danger); margin-top: 0.7rem; font-size: 0.9rem; }
  .result { margin-top: 1rem; padding: 0.9rem; border: 1px solid var(--border); border-radius: 8px; background: var(--surface-soft); }
  .result-label { margin: 0 0 0.5rem; color: var(--muted); font-size: 0.86rem; }
  .result-row { display: flex; gap: 0.5rem; align-items: center; }
  .result-input { flex: 1; min-width: 0; padding: 0.52rem; background: var(--surface); border: 1px solid var(--border); border-radius: 7px; color: var(--accent); font-size: 0.88rem; height: 38px; }
  .list-head { display: flex; justify-content: space-between; align-items: flex-start; margin-bottom: 1rem; gap: 0.75rem; flex-wrap: wrap; }
  .list-head-right { display: flex; gap: 0.5rem; align-items: center; }
  .search-input {
    padding: 0.48rem 0.7rem;
    background: var(--surface-soft);
    border: 1px solid var(--border);
    border-radius: 7px;
    color: var(--text);
    font-size: 0.9rem;
    width: 190px;
    height: 38px;
  }
  .search-input:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
  .link-list { list-style: none; margin: 0; padding: 0; border-top: 1px solid var(--border); }
  .link-item {
    display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem;
    padding: 0.9rem 0; border-bottom: 1px solid var(--border);
  }
  .link-item:last-child { border-bottom: none; }
  .link-main { flex: 1; min-width: 0; }
  .link-code { display: inline-block; color: var(--accent); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-weight: 700; margin-right: 0.5rem; }
  .link-url { color: var(--text); font-size: 0.9rem; word-break: break-all; }
  .link-meta { display: block; margin-top: 0.3rem; color: var(--muted); font-size: 0.8rem; }
  .link-actions { display: flex; gap: 0.45rem; flex-shrink: 0; }
  .loading, .empty { color: var(--muted); padding: 1.1rem 0; text-align: center; border-top: 1px solid var(--border); }
  .pagination {
    display: flex; align-items: center; justify-content: center; gap: 1rem;
    padding-top: 1rem; margin-top: 0.5rem; border-top: 1px solid var(--border);
  }
  .page-info { color: var(--muted); font-size: 0.9rem; }
  .toast {
    position: fixed; top: 5rem; right: 1rem;
    background: var(--surface); color: var(--text); padding: 0.65rem 1.15rem;
    border: 1px solid var(--border); border-radius: 8px; font-size: 0.9rem; z-index: 200; box-shadow: var(--shadow);
  }
  .toast-enter-active, .toast-leave-active { transition: opacity 0.25s, transform 0.25s; }
  .toast-enter-from, .toast-leave-to { opacity: 0; transform: translateX(10px) translateY(0); }
  .modal-overlay {
    position: fixed; inset: 0; background: rgba(0,0,0,0.58); display: flex; align-items: center; justify-content: center;
    z-index: 100; padding: 1rem;
  }
  .modal { background: var(--surface); border: 1px solid var(--border); border-radius: 10px; max-width: 720px; width: 100%; max-height: 85vh; display: flex; flex-direction: column; box-shadow: var(--shadow); }
  .modal-head { display: flex; justify-content: space-between; align-items: center; padding: 1rem 1.2rem; border-bottom: 1px solid var(--border); }
  .modal-head h3 { margin: 0; font-size: 1rem; }
  .modal-close { background: var(--control-bg); border: 1px solid var(--border); border-radius: 6px; color: var(--muted); font-size: 1.2rem; cursor: pointer; line-height: 1; }
  .modal-body { padding: 1.2rem; overflow: auto; }
  .stats-original { color: var(--muted); font-size: 0.9rem; word-break: break-all; margin: 0 0 0.5rem; }
  .stats-total { margin: 0 0 1rem; }
  .visits-table-wrap { overflow-x: auto; }
  .visits-table { width: 100%; border-collapse: collapse; font-size: 0.85rem; }
  .visits-table th, .visits-table td { padding: 0.55rem 0.75rem; text-align: left; border-bottom: 1px solid var(--border); }
  .visits-table th { color: var(--muted); font-weight: 700; }
  .visits-table .ua { max-width: 240px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  @media (max-width: 820px) {
    .home { grid-template-columns: 1fr; }
    .list-head-right { width: 100%; }
    .search-input { flex: 1; width: auto; }
    .link-item { flex-direction: column; }
    .link-actions { flex-wrap: wrap; }
  }
</style>
