<template>
  <div class="app" :data-theme="theme">
    <header class="topbar">
      <div class="brand">
        <img class="brand-mark" src="/favicon.svg" alt="Link WithU" />
        <div>
          <h1>Link WithU</h1>
          <p class="subtitle">一款简约的短链管理器</p>
        </div>
      </div>
      <button class="theme-toggle" @click="toggleTheme" :title="theme === 'dark' ? '切换亮色' : '切换暗色'">
        {{ theme === 'dark' ? 'Light' : 'Dark' }}
      </button>
    </header>
    <main class="main">
      <router-view />
    </main>
    <footer class="footer">
      Made With
      <a href="https://github.com/Explorer-Dong/link-withu" target="_blank" rel="noreferrer">Link WithU</a>
    </footer>
  </div>
</template>

<script setup>
import { ref, watchEffect } from 'vue'

function getInitialTheme() {
  const stored = localStorage.getItem('theme')
  if (stored === 'light' || stored === 'dark') return stored
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark'
}

const theme = ref(getInitialTheme())

watchEffect(() => {
  document.documentElement.setAttribute('data-theme', theme.value)
  localStorage.setItem('theme', theme.value)
})

function toggleTheme() {
  theme.value = theme.value === 'dark' ? 'light' : 'dark'
}
</script>

<style>
  :root, [data-theme="dark"] {
    --bg: #0f1115;
    --surface: #171a21;
    --surface-soft: #1f232c;
    --border: #2f3541;
    --text: #f4f4f5;
    --muted: #a1a7b3;
    --accent: #d1905c;
    --accent-hover: #e0a878;
    --accent-soft: rgba(209, 144, 92, 0.16);
    --control-bg: #222733;
    --control-hover: #2b3140;
    --success: #63b48d;
    --danger: #ee7b73;
    --danger-bg: #3b2324;
    --danger-hover: #4a2a2c;
    --shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }
  [data-theme="light"] {
    --bg: #f7f7f8;
    --surface: #ffffff;
    --surface-soft: #f2f3f5;
    --border: #d9d9dc;
    --text: #1d1f23;
    --muted: #6f737c;
    --accent: #c87945;
    --accent-hover: #ad6739;
    --accent-soft: rgba(200, 121, 69, 0.13);
    --control-bg: #f2f3f5;
    --control-hover: #e8e9ec;
    --success: #2f8f68;
    --danger: #c7473f;
    --danger-bg: #f8e9e7;
    --danger-hover: #f0d8d5;
    --shadow: 0 4px 12px rgba(28, 31, 35, 0.04);
  }
  * { box-sizing: border-box; }
  body {
    margin: 0;
    font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    background: var(--bg);
    color: var(--text);
    min-height: 100vh;
  }
  .app { min-height: 100vh; display: flex; flex-direction: column; }
  .topbar {
    position: sticky;
    top: 0;
    z-index: 20;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.85rem max(1rem, calc((100vw - 1040px) / 2));
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    border-bottom: 1px solid var(--border);
    backdrop-filter: blur(12px);
  }
  .brand { display: flex; align-items: center; gap: 0.75rem; }
  .brand-mark {
    display: block;
    width: 2.3rem;
    height: 2.3rem;
  }
  .brand h1 { margin: 0; font-size: 1rem; font-weight: 700; color: var(--text); }
  .subtitle { margin: 0.15rem 0 0; color: var(--muted); font-size: 0.82rem; }
  .theme-toggle {
    background: var(--control-bg);
    border: 1px solid var(--border);
    border-radius: 0.55rem;
    color: var(--text);
    padding: 0.45rem 0.7rem;
    cursor: pointer;
    font-size: 0.86rem;
    font-weight: 600;
    line-height: 1;
  }
  .theme-toggle:hover { border-color: var(--accent); background: var(--control-hover); }
  .main { flex: 1; max-width: 1040px; margin: 0 auto; padding: 1.6rem 1rem 2.2rem; width: 100%; }
  .footer {
    padding: 0.9rem 1rem 1rem;
    color: var(--muted);
    font-size: 0.85rem;
    text-align: center;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }
  .footer a {
    color: var(--accent);
    font-weight: 700;
    text-decoration: none;
  }
  .footer a:hover { color: var(--accent-hover); }
</style>
