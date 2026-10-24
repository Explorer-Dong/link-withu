<script setup>
import { ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import Icon from "../components/Icon.vue";
import { adminLogin } from "../api/links";
import { site, saveSession } from "../state";
const password = ref(""),
  busy = ref(false),
  error = ref(""),
  visible = ref(false);
const route = useRoute(),
  router = useRouter();
async function login() {
  busy.value = true;
  error.value = "";
  try {
    saveSession(await adminLogin(password.value));
    const next = route.query.redirect;
    router.replace(
      typeof next === "string" &&
        /^\/(?!\/)/.test(next) &&
        !next.startsWith("/login")
        ? next
        : "/",
    );
  } catch (e) {
    error.value = e.message;
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <div class="login-page">
    <div class="login-brand">
      <img
        class="brand"
        src="/favicon.svg?v=lw"
        alt=""
        width="54"
        height="54"
      />{{ site.app_name }}
    </div>
    <section class="login-card">
      <h1>管理员登录</h1>
      <form @submit.prevent="login">
        <label for="password">管理员密码</label>
        <div class="password-field">
          <Icon name="lock" :size="18" /><input
            id="password"
            v-model="password"
            :type="visible ? 'text' : 'password'"
            autocomplete="current-password"
            placeholder="输入管理员密码"
            required
            autofocus
            maxlength="1024"
          /><button
            type="button"
            class="icon-btn"
            :aria-label="visible ? '隐藏密码' : '显示密码'"
            @click="visible = !visible"
          >
            <Icon :name="visible ? 'eye-off' : 'eye'" :size="18" />
          </button>
        </div>
        <p v-if="error" class="error-message" role="alert">{{ error }}</p>
        <button class="btn primary login-submit" :disabled="busy">
          {{ busy ? "正在登录…" : "登录工作空间"
          }}<Icon name="arrow-right" :size="18" />
        </button>
      </form>
    </section>
    <p class="login-footer">
      <a
        href="https://github.com/Explorer-Dong/link-withu"
        target="_blank"
        rel="noreferrer"
        >v{{ site.version }}</a
      >
    </p>
  </div>
</template>
