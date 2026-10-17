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

export async function assertNoOverflow(page) {
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
}
