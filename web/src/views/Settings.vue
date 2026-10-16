<script setup>
import { computed, onMounted, reactive, ref } from "vue";
import { useRouter } from "vue-router";
import Icon from "../components/Icon.vue";
import Modal from "../components/Modal.vue";
import { api, download } from "../api/links";
import { theme, site, notify, number, saveSession, tags } from "../state";
const router = useRouter();
const info = ref(null),
  error = ref(""),
  loading = ref(false),
  saving = ref(false),
  busy = ref(""),
  restoreFile = ref(null),
  restorePreview = ref(null),
  confirmation = ref(""),
  fileInput = ref(null);
const form = reactive({
  app_name: "",
  base_url: "",
  random_code_length: 6,
});
const size = computed(() =>
  info.value
    ? `${(info.value.database_bytes / 1024 / 1024).toFixed(2)} MiB`
    : "—",
);
async function load() {
  loading.value = true;
  error.value = "";
  try {
    info.value = await api("/api/admin/settings");
    for (const key of Object.keys(form)) form[key] = info.value.site[key];
  } catch (e) {
    error.value = e.message;
  } finally {
    loading.value = false;
  }
}
async function save() {
  saving.value = true;
  try {
    const updated = await api("/api/admin/settings", {
      method: "PUT",
      body: JSON.stringify(form),
    });
    site.value.app_name = updated.app_name;
    notify("设置已保存");
  } catch (e) {
    notify(e.message, true);
  } finally {
    saving.value = false;
  }
}
async function exportAllData() {
  busy.value = "backup";
  try {
    await download("/api/admin/backup");
    notify("全部数据已导出");
  } catch (e) {
    notify(e.message, true);
  } finally {
    busy.value = "";
  }
}
function logout() {
  saveSession(null);
  tags.value = [];
  router.replace("/login");
}
async function chooseFile(event) {
  const file = event.target.files?.[0];
  event.target.value = "";
  if (!file || saving.value || busy.value) return;
  if (file.size > info.value.max_restore_bytes) {
    notify("恢复文件不能超过 50 MiB", true);
    return;
  }
  busy.value = "preview";
  try {
    const data = JSON.parse(await file.text());
    if (
      data.format !== "link-withu-backup" ||
      data.version !== 1 ||
      !Array.isArray(data.links) ||
      !Array.isArray(data.visits)
    )
      throw new Error("invalid");
    restoreFile.value = file;
    restorePreview.value = {
      links: data.links.length,
      visits: data.visits.length,
    };
    confirmation.value = "";
  } catch {
    notify("请选择有效的 Link WithU JSON 备份文件", true);
  } finally {
    busy.value = "";
  }
}
async function restore() {
  busy.value = "restore";
  try {
    const result = await api("/api/admin/restore", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Confirm-Restore": "RESTORE",
      },
      body: restoreFile.value,
    });
    restorePreview.value = null;
    restoreFile.value = null;
    await load();
    site.value.app_name = info.value.site.app_name;
    window.dispatchEvent(new Event("links-changed"));
    notify(`已恢复 ${result.links} 个短链、${result.visits} 条访问记录`);
  } catch (e) {
    notify(e.message, true);
  } finally {
    busy.value = "";
  }
}
onMounted(load);
</script>
<template>
  <div class="page-heading">
    <div><h1>设置</h1></div>
  </div>
  <p v-if="error" class="error-message" role="alert">
    {{ error }} <button class="text-btn" @click="load">重试</button>
  </p>
  <div v-if="loading" class="loading-state">正在读取设置…</div>
  <div v-else-if="info" class="settings-grid">
    <div class="settings-column">
      <section class="card settings-card">
        <div class="section-heading">
          <h2><Icon name="settings" :size="18" />站点设置</h2>
        </div>
        <form class="stack-form" @submit.prevent="save">
          <label
            >站点名称<input
              v-model="form.app_name"
              :disabled="saving || !!busy"
              required
              maxlength="60" /></label
          ><label
            >公开访问地址<input
              aria-label="公开访问地址"
              aria-describedby="base-url-help"
              v-model="form.base_url"
              :disabled="saving || !!busy"
              required
              placeholder="https://go.example.com"
              maxlength="2048"
            /><small id="base-url-help">域名及端口，不含路径。</small></label
          >
          <label
            >随机短码长度<input
              v-model.number="form.random_code_length"
              :disabled="saving || !!busy"
              type="number"
              min="4"
              max="16"
              required
          /></label>
          <div class="settings-save">
            <button class="btn primary" :disabled="saving || !!busy">
              {{ saving ? "保存中…" : "保存设置" }}
            </button>
          </div>
        </form>
      </section>
      <section class="card settings-card">
        <div class="section-heading">
          <h2><Icon name="palette" :size="18" />外观</h2>
        </div>
        <div class="theme-options">
          <button
            v-for="option in [
              { key: 'light', label: '浅色' },
              { key: 'dark', label: '深色' },
              { key: 'system', label: '跟随系统' },
            ]"
            :key="option.key"
            :class="['theme-option', { selected: theme === option.key }]"
            :aria-pressed="theme === option.key"
            @click="theme = option.key"
          >
            <span :class="['theme-preview', option.key]"
              ><i></i><i></i><i></i></span
            >{{ option.label }}
          </button>
        </div>
      </section>
      <section class="card settings-card">
        <div class="section-heading">
          <h2><Icon name="lock" :size="18" />访问与安全</h2>
        </div>
        <div class="setting-info-row">
          <span>管理员会话</span><strong>24 小时</strong>
        </div>
        <div class="setting-info-row">
          <span>可信反向代理层数</span
          ><strong>{{ info.trusted_proxy_count }}</strong>
        </div>
        <p class="muted small">
          密码与代理层数通过环境变量配置，修改后重启容器。
        </p>
        <div class="settings-logout">
          <button class="btn" :disabled="saving || !!busy" @click="logout">
            <Icon name="logout" :size="18" />退出登录
          </button>
        </div>
      </section>
    </div>
    <div class="settings-column">
      <section class="card settings-card">
        <div class="section-heading">
          <h2><Icon name="download" :size="18" />数据与备份</h2>
        </div>
        <div class="data-stats">
          <div>
            <strong>{{ number(info.links) }}</strong
            ><small>短链</small>
          </div>
          <div>
            <strong>{{ number(info.visits) }}</strong
            ><small>访问记录</small>
          </div>
          <div>
            <strong>{{ size }}</strong
            ><small>数据库大小</small>
          </div>
        </div>
        <p class="muted small">
          完整 JSON 备份，包含短链、标签、访问记录与站点设置。
        </p>
        <div class="backup-actions">
          <button
            class="btn primary"
            :disabled="saving || !!busy"
            @click="exportAllData"
          >
            <Icon name="download" :size="18" />{{
              busy === "backup" ? "导出中…" : "导出全部数据"
            }}
          </button>
          <input
            ref="fileInput"
            type="file"
            accept=".json,application/json"
            hidden
            aria-label="选择备份文件"
            @change="chooseFile"
          />
          <button
            class="btn"
            :disabled="saving || !!busy"
            @click="fileInput.click()"
          >
            从备份恢复
          </button>
        </div>
      </section>
      <section class="card settings-card about-card">
        <div class="section-heading">
          <h2><Icon name="link" :size="18" />关于</h2>
        </div>
        <div class="setting-info-row">
          <span>版本</span><strong>{{ info.version }}</strong>
        </div>
        <div class="setting-info-row">
          <span>存储</span><strong>SQLite</strong>
        </div>
        <div class="setting-info-row">
          <span>开源协议</span><strong>MIT</strong>
        </div>
        <a
          class="text-btn"
          href="https://github.com/Explorer-Dong/link-withu"
          target="_blank"
          rel="noreferrer"
          >GitHub <Icon name="arrow" :size="15"
        /></a>
      </section>
    </div>
  </div>
  <Modal
    v-if="restorePreview"
    title="确认覆盖恢复"
    @close="!busy && (restorePreview = null)"
    ><div class="restore-warning">
      当前数据将被替换为备份中的 {{ number(restorePreview.links) }} 个短链、{{
        number(restorePreview.visits)
      }}
      条访问记录。
    </div>
    <p class="muted small">
      文件：{{ restoreFile.name }}。请确认已保存当前备份。
    </p>
    <form class="stack-form" @submit.prevent="restore">
      <label
        >输入 RESTORE 确认覆盖<input
          v-model="confirmation"
          placeholder="RESTORE"
          autocomplete="off"
          required
          pattern="RESTORE"
      /></label>
      <div class="modal-actions">
        <button
          type="button"
          class="btn"
          :disabled="!!busy"
          @click="restorePreview = null"
        >
          取消</button
        ><button
          class="btn danger"
          :disabled="confirmation !== 'RESTORE' || !!busy"
        >
          {{ busy === "restore" ? "正在恢复…" : "覆盖并恢复" }}
        </button>
      </div>
    </form></Modal
  >
</template>
