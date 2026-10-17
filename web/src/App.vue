<script setup>
import { onMounted, onUnmounted, watch } from "vue";
import { useRoute } from "vue-router";
import Icon from "./components/Icon.vue";
import { api, refreshTags } from "./api/links";
import { theme, applyTheme, session, site, toast } from "./state";
import "./style.css";
const route = useRoute();
watch(
  [() => route.meta.title, () => site.value.app_name],
  ([pageTitle, siteName]) => {
    document.title = pageTitle ? `${pageTitle} · ${siteName}` : siteName;
  },
  { immediate: true },
);
const media = matchMedia("(prefers-color-scheme: dark)");
const updateTags = async () => {
  try {
    await refreshTags();
  } catch {
    /* views show request failures */
  }
};
onMounted(async () => {
  applyTheme();
  media.addEventListener("change", applyTheme);
  window.addEventListener("links-changed", updateTags);
  try {
    const previous = site.value;
    const result = await api("/api/site");
    if (site.value === previous) site.value = result;
  } catch {
    /* default branding */
  }
  updateTags();
});
onUnmounted(() => {
  media.removeEventListener("change", applyTheme);
  window.removeEventListener("links-changed", updateTags);
});
watch(theme, applyTheme);
watch(session, updateTags);
</script>
<template>
  <div v-if="route.path === '/login'" class="login-shell"><router-view /></div>
  <div v-else class="workspace">
    <aside class="sidebar" aria-label="工作区导航">
      <nav class="main-nav" aria-label="主导航">
        <router-link
          to="/"
          aria-label="访问统计"
          title="访问统计"
          :class="{ active: route.path === '/' }"
        >
          <Icon name="home" :size="24" /><span class="nav-label">访问统计</span>
        </router-link>
        <router-link
          to="/links"
          aria-label="链接管理"
          title="链接管理"
          :class="{ active: route.path === '/links' }"
        >
          <Icon name="links" :size="24" /><span class="nav-label"
            >链接管理</span
          >
        </router-link>
        <router-link
          to="/settings"
          aria-label="设置"
          title="设置"
          :class="{ active: route.path === '/settings' }"
        >
          <Icon name="settings" :size="24" /><span class="nav-label">设置</span>
        </router-link>
      </nav>
    </aside>
    <div class="workspace-main">
      <main class="page"><router-view /></main>
    </div>
  </div>
  <div
    v-if="toast"
    class="toast"
    :class="{ 'toast-error': toast.error }"
    role="status"
  >
    {{ toast.message }}
  </div>
</template>
