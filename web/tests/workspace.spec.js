import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";

async function login(page) {
  await page.goto("/login");
  await page
    .getByLabel("管理员密码", { exact: true })
    .fill("browser-test-only");
  await page.getByRole("button", { name: "登录工作空间" }).click();
  await expect(page).toHaveURL(/\/$/);
}
async function assertNoOverflow(page) {
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
}

test("independent login, protected deep links, themes and logout", async ({
  page,
}, testInfo) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/settings");
  await expect(page).toHaveURL(/\/login\?redirect=/);
  await page.getByLabel("管理员密码", { exact: true }).fill("wrong");
  await page.getByRole("button", { name: "登录工作空间" }).click();
  await expect(page.getByRole("alert")).toHaveText("密码错误");
  await page.screenshot({
    path: testInfo.outputPath("login.png"),
    fullPage: true,
  });
  await page
    .getByLabel("管理员密码", { exact: true })
    .fill("browser-test-only");
  await page.getByRole("button", { name: "登录工作空间" }).click();
  await expect(page).toHaveURL("/settings");
  await expect(page.getByRole("heading", { name: "站点设置" })).toBeVisible();
  await page.reload();
  await expect(page.getByRole("heading", { name: "数据与备份" })).toBeVisible();
  await page.getByRole("button", { name: "深色", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.screenshot({
    path: testInfo.outputPath("settings-dark.png"),
    fullPage: true,
  });
  await page.getByRole("button", { name: "浅色", exact: true }).click();
  await assertNoOverflow(page);
  await page.getByRole("button", { name: "退出登录" }).click();
  await expect(page).toHaveURL("/login");
  await page.goto("/links");
  await expect(page).toHaveURL(/\/login\?redirect=/);
  expect(errors).toEqual([]);
});

test("create, search, tag, edit, analytics, bot filter and delete", async ({
  page,
  request,
}, testInfo) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await login(page);
  await expect(
    page.getByRole("heading", { name: "访问统计", exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "切换主题" })).toHaveCount(0);
  await expect(
    page.getByText(/统计时区|按 .* 统计|浏览器本地时间/),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "全部时间", exact: true }),
  ).toHaveClass(/selected/);
  await expect(page.getByRole("combobox", { name: "统计标签" })).toHaveCount(0);
  await page
    .getByRole("navigation", { name: "主导航" })
    .getByRole("link", { name: "链接管理" })
    .click();
  await expect(page).toHaveURL("/links");
  await page.reload();
  const code = `browser-${testInfo.project.name}`;
  await page.getByRole("button", { name: "更多选项" }).click();
  await page
    .getByLabel("目标链接", { exact: true })
    .fill("https://example.org/original");
  await page.getByLabel("链接名称", { exact: true }).fill("产品更新");
  await page.getByLabel("自定义短码", { exact: true }).fill(code);
  await page.getByLabel("标签", { exact: true }).fill("产品, 博客");
  await page.getByLabel("备注", { exact: true }).fill("分享第一份产品更新");
  await page.getByRole("button", { name: "创建短链", exact: true }).click();
  await expect(
    page.getByRole("article").filter({ hasText: code }),
  ).toBeVisible();
  const card = page.getByRole("article").filter({ hasText: code });
  await card.getByRole("button", { name: "编辑短链" }).click();
  await page
    .getByRole("dialog")
    .getByLabel("目标链接", { exact: true })
    .fill("https://example.org/updated");
  await page
    .getByRole("dialog")
    .getByLabel("标签", { exact: true })
    .fill("产品, 已更新");
  await page.getByRole("button", { name: "保存修改" }).click();
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await expect(card).toContainText("https://example.org/updated");
  await page.getByRole("textbox", { name: "搜索链接" }).fill("不会存在的词");
  await expect(
    page.getByRole("heading", { name: "没有找到匹配的链接" }),
  ).toBeVisible();
  await page.getByRole("textbox", { name: "搜索链接" }).fill(code);
  await expect(card).toBeVisible();
  await card.getByRole("button", { name: "# 已更新" }).click();
  await expect(page).toHaveURL(/tag=/);
  await expect(card).toBeVisible();
  await page.screenshot({
    path: testInfo.outputPath("links.png"),
    fullPage: true,
  });
  await assertNoOverflow(page);
  const res = await request.get(`/r/${code}`, {
    maxRedirects: 0,
    headers: {
      "user-agent": "Mozilla/5.0 (Windows NT 10.0) Chrome/130.0",
      referer: "https://blog.example.org/post",
    },
  });
  expect(res.status()).toBe(302);
  expect(res.headers().location).toBe("https://example.org/updated");
  await request.get(`/r/${code}`, {
    maxRedirects: 0,
    headers: { "user-agent": "Googlebot/2.1" },
  });
  await card.getByRole("link", { name: `访问统计：${code}` }).click();
  await expect(page.getByRole("heading", { name: "访问趋势" })).toBeVisible();
  await expect(
    page.getByRole("cell", { name: "blog.example.org", exact: true }),
  ).toBeVisible();
  await page.getByRole("combobox", { name: "统计标签" }).selectOption("已更新");
  await expect(page.getByRole("heading", { name: "访问趋势" })).toBeVisible();
  await expect(page.getByRole("alert")).not.toBeVisible();
  await page.getByLabel("排除疑似机器人").check();
  await expect(
    page.getByRole("cell", { name: "疑似机器人", exact: true }),
  ).not.toBeVisible();
  await page.getByRole("button", { name: "近 7 天", exact: true }).click();
  await page.screenshot({
    path: testInfo.outputPath("analytics.png"),
    fullPage: true,
  });
  await assertNoOverflow(page);
  await page.reload();
  await expect(page.getByRole("heading", { name: "访问趋势" })).toBeVisible();
  await page
    .getByRole("navigation", { name: "主导航" })
    .getByRole("link", { name: "链接管理" })
    .click();
  await page
    .getByRole("article")
    .filter({ hasText: code })
    .getByRole("button", { name: "删除短链" })
    .click();
  await page.getByRole("button", { name: "确认删除" }).click();
  await expect(
    page.getByRole("article").filter({ hasText: code }),
  ).not.toBeVisible();
  expect(errors).toEqual([]);
});

test("settings save, full data export and validated restore", async ({
  page,
}, testInfo) => {
  await login(page);
  await page
    .getByRole("navigation", { name: "主导航" })
    .getByRole("link", { name: "设置" })
    .click();
  await page.getByLabel("站点名称", { exact: true }).fill("我的短链空间");
  await page
    .getByLabel("公开访问地址", { exact: true })
    .fill("http://127.0.0.1:5202");
  await expect(page.getByLabel("统计时区", { exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page.getByRole("status")).toContainText("设置已保存");
  await page.reload();
  await expect(page.getByLabel("站点名称", { exact: true })).toHaveValue(
    "我的短链空间",
  );
  const pending = page.waitForEvent("download");
  await page.getByRole("button", { name: "导出全部数据", exact: true }).click();
  const backup = await pending;
  const backupPath = await backup.path();
  const data = JSON.parse(await readFile(backupPath, "utf8"));
  expect(data.settings.timezone).toBe("Asia/Shanghai");
  await page.getByLabel("站点名称", { exact: true }).fill("临时改名");
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page.getByRole("status")).toContainText("设置已保存");
  await page.getByLabel("选择备份文件").setInputFiles(backupPath);
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.getByRole("button", { name: "覆盖并恢复" })).toBeDisabled();
  await page.getByLabel("输入 RESTORE 确认覆盖").fill("RESTORE");
  await page.getByRole("button", { name: "覆盖并恢复" }).click();
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await expect(page.getByLabel("站点名称", { exact: true })).toHaveValue(
    "我的短链空间",
  );
  await page.screenshot({
    path: testInfo.outputPath("settings.png"),
    fullPage: true,
  });
  await assertNoOverflow(page);
});

test("ten links per page, filter resets and deleting the last page", async ({
  page,
  request,
}, testInfo) => {
  const loginResponse = await request.post("/api/admin/login", {
    data: { password: "browser-test-only" },
  });
  const { token } = await loginResponse.json();
  const headers = { Authorization: `Bearer ${token}` };
  const prefix = `paging-${testInfo.project.name}`;
  const codes = Array.from(
    { length: 11 },
    (_, i) => `${prefix}-${String(i).padStart(2, "0")}`,
  );
  await page.clock.install();
  try {
    for (const [index, code] of codes.entries()) {
      const response = await request.post("/api/links", {
        headers,
        data: {
          short_code: code,
          original_url: "https://example.org/pagination",
          tags: index === 0 ? [prefix] : [],
        },
      });
      expect(response.status()).toBe(201);
    }
    await login(page);
    await page.goto("/links");
    await expect(page.getByRole("article")).toHaveCount(10);
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1000));
    await page.getByRole("textbox", { name: "搜索链接" }).fill(prefix);
    await page
      .getByRole("combobox", { name: "排序方式" })
      .selectOption("oldest");
    const pagination = page.getByLabel("链接分页");
    await expect(page.getByRole("article")).toHaveCount(10);
    await expect(page.locator(".page-heading")).not.toContainText("个链接");
    await expect(pagination).toContainText("第 1 / 2 页");
    await expect(
      pagination.getByRole("button", { name: "上一页" }),
    ).toBeDisabled();
    await pagination.getByRole("button", { name: "下一页" }).click();
    await expect(page.getByRole("article")).toHaveCount(1);
    await expect(page.getByRole("article")).toContainText(codes[10]);
    await expect(
      pagination.getByRole("button", { name: "下一页" }),
    ).toBeDisabled();
    await assertNoOverflow(page);

    await page
      .getByRole("combobox", { name: "排序方式" })
      .selectOption("newest");
    await expect(pagination).toContainText("第 1 / 2 页");
    await expect(page.getByRole("article")).toHaveCount(10);
    await expect(page.getByRole("article").first()).toContainText(codes[10]);
    await pagination.getByRole("button", { name: "下一页" }).click();
    await expect(page.getByRole("article")).toHaveCount(1);
    // Let the original search debounce expire after sorting and changing pages.
    await page.clock.runFor(300);
    await expect(pagination).toContainText("第 2 / 2 页");
    await expect(page.getByRole("article")).toHaveCount(1);
    await page.clock.resume();
    await page.getByRole("textbox", { name: "搜索链接" }).fill(codes[5]);
    await expect(page.getByRole("article")).toHaveCount(1);
    await expect(page.getByRole("article")).toContainText(codes[5]);
    await expect(pagination).toHaveCount(0);

    await page.getByRole("textbox", { name: "搜索链接" }).fill(prefix);
    await expect(page.getByRole("article")).toHaveCount(10);
    await pagination.getByRole("button", { name: "下一页" }).click();
    await expect(page.getByRole("article")).toHaveCount(1);
    await page.getByRole("combobox", { name: "筛选标签" }).selectOption(prefix);
    await expect(page.getByRole("article")).toHaveCount(1);
    await expect(page.getByRole("article")).toContainText(codes[0]);
    await expect(pagination).toHaveCount(0);
    await page.getByRole("combobox", { name: "筛选标签" }).selectOption("");
    await expect(page.getByRole("article")).toHaveCount(10);
    await pagination.getByRole("button", { name: "下一页" }).click();
    await expect(page.getByRole("article")).toHaveCount(1);
    await page
      .getByRole("article")
      .getByRole("button", { name: "删除短链" })
      .click();
    await page.getByRole("button", { name: "确认删除" }).click();
    await expect(page.getByRole("dialog")).not.toBeVisible();
    await expect(page.getByRole("article")).toHaveCount(10);
    await expect(pagination).toHaveCount(0);
  } finally {
    for (const code of codes)
      await request.delete(`/api/links/${code}`, { headers });
  }
});
