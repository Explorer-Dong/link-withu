<template>
  <div class="app">
    <header class="header">
      <h1>短链服务</h1>
      <p class="subtitle">生成短链接，查看访问统计</p>
    </header>
    <main class="main">
      <router-view />
    </main>
    <footer class="footer">
      <a href="/api/health" target="_blank">API 健康检查</a>
    </footer>
  </div>
</template>

<script setup>
</script>

<style>
  :root {
    --bg: #0f0f12;
    --surface: #18181c;
    --border: #2a2a30;
    --text: #e4e4e7;
    --muted: #71717a;
    --accent: #a78bfa;
    --accent-hover: #c4b5fd;
    --success: #34d399;
    --danger: #f87171;
  }
  * { box-sizing: border-box; }
  body { margin: 0; font-family: 'Segoe UI', system-ui, sans-serif; background: var(--bg); color: var(--text); min-height: 100vh; }
  .app { min-height: 100vh; display: flex; flex-direction: column; }
  .header { padding: 2rem 1rem; text-align: center; border-bottom: 1px solid var(--border); }
  .header h1 { margin: 0; font-size: 1.75rem; font-weight: 600; color: var(--accent); }
  .subtitle { margin: 0.5rem 0 0; color: var(--muted); font-size: 0.95rem; }
  .main { flex: 1; max-width: 720px; margin: 0 auto; padding: 2rem 1rem; width: 100%; }
  .footer { padding: 1rem; text-align: center; color: var(--muted); font-size: 0.85rem; border-top: 1px solid var(--border); }
  .footer a { color: var(--accent); }
</style>
