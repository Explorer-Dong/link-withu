import { test as base, expect } from "@playwright/test";

export const test = base.extend({
  page: async ({ page }, use) => {
    const errors = [];
    const record = (error) => errors.push(error.message);
    page.on("pageerror", record);
    await use(page);
    page.off("pageerror", record);
    expect(errors, "浏览器不应有未捕获的 JavaScript 异常").toEqual([]);
  },
});

export async function adminHeaders(request) {
  const response = await request.post("/api/admin/login", {
    data: { password: "browser-test-only" },
  });
  expect(response.ok()).toBe(true);
  return { Authorization: `Bearer ${(await response.json()).token}` };
}

export async function resetData({ request }) {
  const headers = await adminHeaders(request);
  const reset = await request.post("/api/admin/restore", {
    headers: { ...headers, "X-Confirm-Restore": "RESTORE" },
    data: {
      format: "link-withu-backup",
      version: 1,
      exported_at: new Date().toISOString(),
      settings: {
        app_name: "Link WithU",
        base_url: "http://127.0.0.1:5202",
        random_code_length: 6,
      },
      links: [],
      visits: [],
    },
  });
  expect(reset.ok()).toBe(true);
}

export async function seedAnalytics(request) {
  const headers = await adminHeaders(request);
  const snapshot = await (
    await request.get("/api/admin/backup", { headers })
  ).json();
  snapshot.links = Array.from({ length: 16 }, (_, i) => ({
    id: i + 1,
    short_code: `item-${i + 1}`,
    original_url: `https://example.com/${i + 1}`,
    title: `链接 ${i + 1}`,
    note: "",
    tags: [`tag-${i + 1}`],
    is_custom: true,
    created_at: "2026-01-01T00:00:00Z",
  }));
  snapshot.visits = snapshot.links.map((link, i) => ({
    id: i + 1,
    short_link_id: link.id,
    visited_at: `2026-01-0${i === 15 ? 2 : 1}T04:00:00Z`,
    ip: `8.8.8.${i + 1}`,
    country: ["美国", "中国", "英国"][i % 3],
    region: "",
    city: `城市 ${i + 1}`,
    user_agent: "test",
    referrer_domain: `source-${i + 1}.example`,
    device: ["desktop", "mobile", "tablet", "unknown"][i % 4],
    browser: `Browser-${i + 1}`,
    is_bot: false,
  }));
  const result = await request.post("/api/admin/restore", {
    headers: { ...headers, "X-Confirm-Restore": "RESTORE" },
    data: snapshot,
  });
  expect(result.ok()).toBe(true);
}

export async function login(page) {
  await page.goto("/login");
  await page
    .getByLabel("管理员密码", { exact: true })
    .fill("browser-test-only");
  await page.getByRole("button", { name: "登录工作空间" }).click();
  await expect(page).toHaveURL(/\/$/);
}

export async function navigate(page, name) {
  await page
    .getByRole("navigation", { name: "主导航" })
    .getByRole("link", { name })
    .click();
}

export function rangeDate(daysAgo = 0) {
  return new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date(Date.now() - daysAgo * 86400000));
}

export async function selectRange(page, days) {
  await page.locator(".date-range-picker > summary").click();
  if (!days) {
    await page.getByRole("button", { name: "重置", exact: true }).click();
    return;
  }
  await page.getByLabel("开始日期", { exact: true }).fill(rangeDate(days - 1));
  await page.getByLabel("结束日期", { exact: true }).fill(rangeDate());
  await page.getByRole("button", { name: "应用", exact: true }).click();
}

export async function assertNoOverflow(page) {
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
}
