import { defineConfig, devices } from "@playwright/test";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const database = join(
  mkdtempSync(join(tmpdir(), "link-withu-browser-")),
  "test.db",
);
export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  reporter: "list",
  use: { baseURL: "http://127.0.0.1:5202", trace: "retain-on-failure" },
  projects: [
    {
      name: "desktop",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1440, height: 1024 },
      },
    },
    {
      name: "mobile",
      use: { ...devices["iPhone 13"], defaultBrowserType: "chromium" },
    },
  ],
  webServer: {
    command:
      "uv run --project ../api uvicorn app.main:app --app-dir ../api --host 127.0.0.1 --port 5202",
    url: "http://127.0.0.1:5202/api/health",
    reuseExistingServer: false,
    env: {
      ADMIN_PASSWORD: "browser-test-only",
      DATABASE_URL: `sqlite+aiosqlite:///${database}`,
      BASE_URL: "http://127.0.0.1:5202",
      TZ: "Asia/Shanghai",
      FRONTEND_DIST: resolve("dist"),
    },
  },
});
