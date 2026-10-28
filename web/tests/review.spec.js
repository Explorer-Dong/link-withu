import { expect } from "@playwright/test";
import {
  test,
  adminHeaders,
  assertNoOverflow,
  login,
  navigate,
  resetData,
  selectRange,
  rangeDate,
  seedAnalytics,
} from "./helpers";

test.beforeEach(resetData);

test("API Key can be generated, rotated and revoked from settings", async ({
  page,
  request,
}) => {
  await login(page);
  await navigate(page, "设置");
  await page.getByRole("button", { name: "生成 API Key", exact: true }).click();
  const key = await page.getByLabel("新 API Key").inputValue();
  expect(key).toMatch(/^lwu_[A-Za-z0-9_-]{43}$/);
  expect(
    (
      await request.get("/api/links", {
        headers: { Authorization: `Bearer ${key}` },
      })
    ).status(),
  ).toBe(200);
  await page.reload();
  await expect(page.getByLabel("新 API Key")).toHaveCount(0);
  await page.route("**/api/admin/api-key", (route) =>
    route.fulfill({ status: 503, json: { detail: "暂时不可用" } }),
  );
  await page.getByRole("button", { name: "重新生成 API Key" }).click();
  await expect(page.getByText("暂时不可用")).toBeVisible();
  expect(
    (
      await request.get("/api/links", {
        headers: { Authorization: `Bearer ${key}` },
      })
    ).status(),
  ).toBe(200);
  await page.unroute("**/api/admin/api-key");
  await page.getByRole("button", { name: "重新生成 API Key" }).click();
  const replacement = await page.getByLabel("新 API Key").inputValue();
  expect(replacement).not.toBe(key);
  expect(
    (
      await request.get("/api/links", {
        headers: { Authorization: `Bearer ${key}` },
      })
    ).status(),
  ).toBe(401);
  expect(
    (
      await request.get("/api/links", {
        headers: { Authorization: `Bearer ${replacement}` },
      })
    ).status(),
  ).toBe(200);
  await assertNoOverflow(page);
  await page.getByRole("button", { name: "撤销 API Key" }).click();
  await expect(page.getByLabel("新 API Key")).toHaveCount(0);
  expect(
    (
      await request.get("/api/links", {
        headers: { Authorization: `Bearer ${replacement}` },
      })
    ).status(),
  ).toBe(401);
});

test("API documentation uses the current OpenAPI description and handles a failed loader", async ({
  page,
}) => {
  const cdn =
    "https://cdn.jsdelivr.net/npm/@scalar/api-reference@1.75.1/esm.js";
  await page.route(cdn, (route) =>
    route.fulfill({
      contentType: "text/javascript",
      headers: { "access-control-allow-origin": "*" },
      body: `export async function createApiReference(selector, config) {
        const document = await (await fetch(config.url)).json();
        globalThis.referenceConfig = config;
        globalThis.referenceDocument = document;
        window.document.querySelector(selector).textContent = document.info.title;
      }`,
    }),
  );
  await page.goto("/docs");
  await expect(page.locator("#api-reference")).toHaveText("Link WithU");
  expect(await page.evaluate(() => window.referenceConfig.persistAuth)).toBe(
    false,
  );
  expect(await page.evaluate(() => window.referenceConfig.agent.disabled)).toBe(
    true,
  );
  expect(await page.evaluate(() => window.referenceConfig.servers[0].url)).toBe(
    "http://127.0.0.1:5202",
  );
  expect(
    await page.evaluate(
      () =>
        !!window.referenceDocument.paths["/api/links/by-code/{short_code}"].get,
    ),
  ).toBe(true);
  await page.unroute(cdn);
  await page.route(cdn, (route) => route.abort());
  await page.reload();
  await expect(page.locator("#api-reference")).toContainText(
    "接口文档加载失败",
  );
});

test("locations show beside IP addresses and share analytics filters", async ({
  page,
  request,
}) => {
  await seedAnalytics(request);
  await login(page);
  const locations = page.locator(".breakdown").filter({
    has: page.getByRole("button", { name: "地理位置", exact: true }),
  });
  await expect(locations.locator(".breakdown-row")).toHaveCount(3);
  await page.goto("/?short_code=item-1");
  await expect(page.locator(".visit-details th")).toHaveText([
    "访问时间",
    "IP",
    "地址",
    "来源",
    "设备 / 浏览器",
    "类型",
  ]);
  const row = page.locator(".visit-details tbody tr").first();
  await expect(row.locator("td").nth(1)).toHaveText("8.8.8.1");
  await expect(row.locator("td").nth(2)).toHaveText("美国 城市 1");
  await expect(locations).toContainText("美国 城市 1");
  await page.getByRole("button", { name: "地理位置", exact: true }).click();
  await expect(page.getByRole("dialog").locator(".detail-row")).toHaveCount(1);
  await expect(page.getByRole("dialog")).toContainText("美国 城市 1");
  await page.keyboard.press("Escape");
  await assertNoOverflow(page);
});

test("resolved locations refresh without replacing a newer selection", async ({
  page,
  request,
}) => {
  await seedAnalytics(request);
  let pending = true;
  await page.route(
    (url) => url.pathname === "/api/analytics",
    async (route) => {
      const response = await route.fetch();
      const data = await response.json();
      if (pending) {
        data.summary.pending_locations = 1;
        data.locations = [{ name: "未知", count: 16 }];
      }
      await route.fulfill({ response, json: data });
    },
  );
  await page.route(
    (url) =>
      url.pathname === "/api/analytics/breakdown" &&
      url.searchParams.get("kind") === "locations",
    async (route) => {
      const url = new URL(route.request().url());
      const groups = Array.from({ length: 6 }, (_, i) => ({
        name: `已解析位置 ${i + 1}`,
        count: 1,
      }));
      if (pending) groups.push({ name: "未知", count: 1 });
      const skip = Number(url.searchParams.get("skip"));
      await route.fulfill({
        json: { total: groups.length, items: groups.slice(skip, skip + 6) },
      });
    },
  );
  await login(page);
  const locations = page.locator(".breakdown").filter({
    has: page.getByRole("button", { name: "地理位置", exact: true }),
  });
  await expect(locations).toContainText("未知");
  await page.locator(".chart-point").first().hover();
  await expect(page.getByRole("tooltip")).toBeVisible();
  const refresh = await page.waitForResponse(
    (response) => new URL(response.url()).pathname === "/api/analytics",
  );
  await refresh.finished();
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
  await expect(page.getByRole("tooltip")).toBeVisible();
  await page.getByRole("button", { name: "地理位置", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.locator(".detail-row")).toHaveCount(6);
  await dialog.getByRole("button", { name: "下一页" }).click();
  await expect(dialog.locator(".detail-row")).toHaveCount(1);
  await expect(dialog).toContainText("未知");
  pending = false;
  await expect(dialog.locator(".detail-row")).toHaveCount(6, { timeout: 7000 });
  await expect(dialog).toContainText("第 1 / 1 页");
  await dialog.getByRole("button", { name: "关闭", exact: true }).click();
  await expect(locations).toContainText("城市", { timeout: 7000 });
  await page.goto("/?short_code=item-2");
  await expect(page.locator(".visit-details tbody")).toContainText(
    "中国 城市 2",
  );
  await expect(locations).toContainText("中国 城市 2");
});

function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("a late initial site response cannot undo a saved name", async ({
  page,
}) => {
  const captured = deferred(),
    release = deferred();
  await page.route("**/api/site", async (route) => {
    const response = await route.fetch();
    captured.resolve();
    await release.promise;
    await route.fulfill({ response });
  });
  try {
    await login(page);
    await captured.promise;
    await navigate(page, "设置");
    await page.getByLabel("站点名称", { exact: true }).fill("  最终站点  ");
    await page.getByRole("button", { name: "保存设置" }).click();
    await expect(page).toHaveTitle("设置 · 最终站点");
    const delivered = page.waitForResponse(
      (r) => new URL(r.url()).pathname === "/api/site",
    );
    release.resolve();
    await delivered;
    await page.evaluate(() => new Promise(requestAnimationFrame));
    await expect(page).toHaveTitle("设置 · 最终站点");
    await expect(page.getByLabel("站点名称", { exact: true })).toHaveValue(
      "最终站点",
    );
    await expect(page).toHaveTitle("设置 · 最终站点");
    await page.getByRole("button", { name: "退出登录" }).click();
    await expect(page.locator(".login-brand")).toHaveText("最终站点");
  } finally {
    release.resolve();
  }
});

test("late tag refreshes cannot remove a newly created tag", async ({
  page,
}) => {
  await login(page);
  await navigate(page, "链接");
  await expect(
    page.getByRole("heading", { name: "从你的第一个链接开始" }),
  ).toBeVisible();
  const captured = deferred(),
    release = deferred();
  let first = true;
  await page.route("**/api/links/tags", async (route) => {
    if (!first) return route.continue();
    first = false;
    const response = await route.fetch();
    captured.resolve();
    await release.promise;
    await route.fulfill({ response });
  });
  try {
    await page.evaluate(() => window.dispatchEvent(new Event("links-changed")));
    await captured.promise;
    await page.getByRole("button", { name: "更多选项" }).click();
    await page.getByLabel("目标链接", { exact: true }).fill("example.org");
    await page.getByLabel("标签", { exact: true }).fill("新标签");
    await page.getByRole("button", { name: "创建短链", exact: true }).click();
    const option = page
      .getByRole("combobox", { name: "筛选标签" })
      .locator("option", { hasText: "新标签" });
    await expect(option).toHaveCount(1);
    const delivered = page.waitForResponse(
      (r) => new URL(r.url()).pathname === "/api/links/tags",
    );
    release.resolve();
    await delivered;
    await expect(option).toHaveCount(1);
    await page
      .getByRole("combobox", { name: "筛选标签" })
      .selectOption("新标签");
    await expect(page.getByRole("article")).toHaveCount(1);
  } finally {
    release.resolve();
  }
});

test("creation errors, clipboard, modal cancellation and deletion retry", async ({
  page,
}) => {
  await login(page);
  await navigate(page, "链接");
  let failCreate = true;
  await page.route("**/api/links", (route) => {
    if (route.request().method() === "POST" && failCreate) {
      failCreate = false;
      return route.fulfill({
        status: 500,
        json: { detail: [{ msg: "暂时无法创建" }] },
      });
    }
    return route.continue();
  });
  await page.getByLabel("目标链接", { exact: true }).fill("example.org/retry");
  await page.getByRole("button", { name: "创建短链", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("暂时无法创建");
  await expect(page.getByLabel("目标链接", { exact: true })).toHaveValue(
    "example.org/retry",
  );
  await page.getByRole("button", { name: "创建短链", exact: true }).click();
  const card = page.getByRole("article");
  await expect(card).toHaveCount(1);
  const shortUrl = await page.locator(".created-result a").getAttribute("href");
  expect(shortUrl).toMatch(/\/r\/[A-Za-z0-9]{6}$/);
  await page.evaluate(() =>
    Object.defineProperty(navigator.clipboard, "writeText", {
      configurable: true,
      value: async (text) => {
        window.copiedText = text;
      },
    }),
  );
  await page.getByRole("button", { name: "复制链接", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("已复制到剪贴板");
  expect(await page.evaluate(() => window.copiedText)).toBe(shortUrl);
  await card.getByRole("button", { name: "复制短链" }).click();
  expect(await page.evaluate(() => window.copiedText)).toBe(shortUrl);
  await page.evaluate(() =>
    Object.defineProperty(navigator.clipboard, "writeText", {
      configurable: true,
      value: async () => {
        throw new Error("Permission denied");
      },
    }),
  );
  await card.getByRole("button", { name: "复制短链" }).click();
  await expect(page.getByRole("status")).toContainText("复制失败");
  const edit = card.getByRole("button", { name: "编辑短链" });
  await edit.click();
  await page
    .getByRole("dialog")
    .getByLabel("链接名称", { exact: true })
    .fill("未保存");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await expect(edit).toBeFocused();
  await expect(card).not.toContainText("未保存");
  await edit.click();
  await page.getByRole("button", { name: "关闭", exact: true }).click();
  let failEdit = true;
  await page.route("**/api/links/*", (route) => {
    if (route.request().method() === "PUT" && failEdit) {
      failEdit = false;
      return route.fulfill({
        status: 409,
        json: { detail: "暂时无法保存修改" },
      });
    }
    return route.continue();
  });
  await edit.click();
  await page
    .getByRole("dialog")
    .getByLabel("链接名称", { exact: true })
    .fill("重试后的修改");
  await page.getByRole("button", { name: "保存修改" }).click();
  await expect(page.getByRole("status")).toContainText("暂时无法保存修改");
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByRole("button", { name: "保存修改" }).click();
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await expect(card).toContainText("重试后的修改");
  await card.getByRole("button", { name: "删除短链" }).click();
  await page.getByRole("button", { name: "取消", exact: true }).click();
  await expect(card).toBeVisible();
  let failDelete = true;
  await page.route("**/api/links/*", (route) => {
    if (route.request().method() === "DELETE" && failDelete) {
      failDelete = false;
      return route.fulfill({ status: 500, body: "temporary failure" });
    }
    return route.continue();
  });
  await card.getByRole("button", { name: "删除短链" }).click();
  await page.getByRole("button", { name: "确认删除" }).click();
  await expect(page.getByRole("status")).toContainText("请求失败（500）");
  await expect(page.getByRole("dialog")).toBeVisible();
  await page.getByRole("button", { name: "确认删除" }).click();
  await expect(card).toHaveCount(0);
  await expect(page.locator(".created-result")).not.toBeVisible();
  await assertNoOverflow(page);
});

test("analytics ranges, rankings, tag groups, bot filtering and visit pages", async ({
  page,
  request,
}) => {
  const headers = await adminHeaders(request);
  const links = [];
  for (const code of ["history", "other"]) {
    const r = await request.post("/api/links", {
      headers,
      data: {
        original_url: "https://example.org",
        short_code: code,
        tags: [code],
      },
    });
    expect(r.ok()).toBe(true);
    links.push(await r.json());
  }
  const snapshot = await (
    await request.get("/api/admin/backup", { headers })
  ).json();
  snapshot.visits = Array.from({ length: 65 }, (_, i) => ({
    id: i + 1,
    short_link_id: links[i < 63 ? 0 : 1].id,
    visited_at: new Date(
      Date.now() -
        (i === 60 ? 10 : i === 61 ? 50 : i === 62 ? 120 : 0) * 86400000,
    ).toISOString(),
    is_bot: i >= 55 && i < 60,
    ip: "127.0.0.1",
    user_agent: "browser-review",
    device: "desktop",
    browser: "Firefox",
  }));
  expect(
    (
      await request.post("/api/admin/restore", {
        headers: { ...headers, "X-Confirm-Restore": "RESTORE" },
        data: snapshot,
      })
    ).ok(),
  ).toBe(true);
  await login(page);
  await page.goto("/?short_code=history");
  const details = page.locator(".visit-details");
  await expect(details).toContainText("63 条");
  await expect(details.locator("tbody tr")).toHaveCount(50);
  await details.getByRole("button", { name: "下一页" }).click();
  await expect(details).toContainText("第 2 / 2 页");
  await expect(details.locator("tbody tr")).toHaveCount(13);
  await details.getByRole("button", { name: "上一页" }).click();
  await expect(details.locator("tbody tr")).toHaveCount(50);
  for (const [days, count] of [
    [7, 60],
    [30, 61],
    [90, 62],
    [0, 63],
  ]) {
    await selectRange(page, days);
    await expect(page.locator(".trend-card")).toContainText(
      `共 ${count} 次访问`,
    );
  }
  await details.getByRole("button", { name: "下一页" }).click();
  await page.getByLabel("排除疑似机器人").check();
  await expect(details).toContainText("58 条");
  await expect(details).toContainText("第 1 / 2 页");
  await expect(details.locator("tbody tr")).toHaveCount(50);
  await expect(page.locator(".trend-card")).toContainText("共 58 次访问");
  await page.getByRole("button", { name: "查看全部链接" }).click();
  await page.getByRole("combobox", { name: "统计标签" }).selectOption("other");
  await expect(page.locator(".trend-card")).toContainText("共 2 次访问");
  await page.locator(".ranking-row").filter({ hasText: "other" }).click();
  await expect(page).toHaveURL(/short_code=other/);
  await expect(details).toContainText("2 条");
  await page.getByRole("button", { name: "查看全部链接" }).click();
  await page.locator(".group-row").filter({ hasText: "history" }).click();
  await expect(page).toHaveURL(/tag=history/);
  await expect(page.locator(".trend-card")).toContainText("共 58 次访问");
  await assertNoOverflow(page);
});

test("three-row summaries open six-row pages and preserve filters", async ({
  page,
  request,
}) => {
  await seedAnalytics(request);
  await login(page);
  await expect(
    page.locator(".breakdown").first().locator(".breakdown-row"),
  ).toHaveCount(3);
  await expect(page.locator(".ranking-row")).toHaveCount(3);
  await expect(page.locator(".group-row")).toHaveCount(3);
  await expect(page.getByText("访问占比", { exact: true })).toHaveCount(0);
  for (const title of [
    "访问来源",
    "浏览器",
    "地理位置",
    "热门短链",
    "标签分组",
  ]) {
    await page.getByRole("button", { name: title, exact: true }).click();
    const dialog = page.getByRole("dialog");
    await expect(dialog.locator(".detail-row")).toHaveCount(6);
    await expect(dialog).toContainText("共 16 项");
    await dialog.getByRole("button", { name: "下一页" }).click();
    await expect(dialog.locator(".detail-row")).toHaveCount(6);
    await expect(dialog).toContainText("第 2 / 3 页");
    await expect(dialog.locator(".rank-number").first()).toHaveText("07");
    await dialog.getByRole("button", { name: "下一页" }).click();
    await expect(dialog.locator(".detail-row")).toHaveCount(4);
    await expect(dialog).toContainText("第 3 / 3 页");
    await expect(dialog.locator(".rank-number").first()).toHaveText("13");
    await expect(dialog.getByRole("button", { name: "下一页" })).toBeDisabled();
    await dialog.getByRole("button", { name: "上一页" }).click();
    await expect(dialog).toContainText("第 2 / 3 页");
    await dialog.getByRole("button", { name: "上一页" }).click();
    await expect(dialog.locator(".detail-row")).toHaveCount(6);
    await dialog.getByRole("button", { name: "关闭", exact: true }).click();
  }
  await page.getByRole("button", { name: "设备分布", exact: true }).click();
  await expect(page.getByRole("dialog").locator(".detail-row")).toHaveCount(4);
  await expect(page.getByRole("dialog")).toContainText("未知");
  await page.keyboard.press("Escape");
  await page.locator(".date-range-picker > summary").click();
  await page.getByLabel("开始日期", { exact: true }).fill("2026-01-02");
  await page.getByLabel("结束日期", { exact: true }).fill("2026-01-01");
  await page.getByRole("button", { name: "应用", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "开始日期不能晚于结束日期",
  );
  await page.getByLabel("开始日期", { exact: true }).fill("2026-01-01");
  await page.getByRole("button", { name: "应用", exact: true }).click();
  await expect(page.locator(".trend-card")).toContainText(
    "2026-01-01 至 2026-01-01 · 共 15 次访问",
  );
  await page.getByRole("button", { name: "访问来源", exact: true }).click();
  await expect(page.getByRole("dialog")).toContainText("共 15 项");
  await page.keyboard.press("Escape");
  await page.locator(".date-range-picker > summary").click();
  await page.getByRole("button", { name: "重置", exact: true }).click();
  await expect(page.locator(".trend-card")).toContainText("共 16 次访问");
  await assertNoOverflow(page);
});

test("chart hover, touch and keyboard reveal dates and visits with particles", async ({
  page,
  request,
}) => {
  await seedAnalytics(request);
  await login(page);
  await page.locator(".date-range-picker > summary").click();
  await page.getByLabel("开始日期", { exact: true }).fill("2026-01-01");
  await page.getByLabel("结束日期", { exact: true }).fill("2026-01-02");
  await page.getByRole("button", { name: "应用", exact: true }).click();
  const chart = page.locator(".trend-chart");
  await expect(chart.locator("polyline")).toBeVisible();
  await chart.locator("svg").hover({ position: { x: 70, y: 80 } });
  await expect(chart.getByRole("tooltip")).toContainText("2026-01-01");
  await expect(chart.getByRole("tooltip")).toContainText("15 次访问");
  await expect(chart.locator(".chart-particle")).toHaveCount(8);
  await chart.focus();
  await page.keyboard.press("ArrowRight");
  await expect(chart.getByRole("tooltip")).toContainText("2026-01-02");
  await expect(chart.getByRole("tooltip")).toContainText("1 次访问");
  await page.keyboard.press("Escape");
  await expect(chart.getByRole("tooltip")).toHaveCount(0);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await chart.locator("svg").dispatchEvent("pointerdown", {
    clientX: (await chart.boundingBox()).x + 70,
  });
  await expect(chart.getByRole("tooltip")).toBeVisible();
  await expect(chart.locator(".chart-particle").first()).toBeHidden();
  await page.locator(".date-range-picker > summary").click();
  await page.getByLabel("结束日期", { exact: true }).fill("2026-01-09");
  await page.getByRole("button", { name: "应用", exact: true }).click();
  await expect(chart.locator(".chart-point")).toHaveCount(9);
  const positions = [];
  for (const index of [0, 1, 7, 8]) {
    const point = chart.locator(".chart-point").nth(index);
    await point.hover();
    const tooltip = chart.getByRole("tooltip");
    await expect(tooltip).toContainText(
      `2026-01-${String(index + 1).padStart(2, "0")}`,
    );
    const bubble = await tooltip.boundingBox();
    const dot = await point.boundingBox();
    const centerX = dot.x + dot.width / 2;
    expect(bubble.x).toBeLessThan(centerX);
    expect(bubble.x + bubble.width).toBeGreaterThan(centerX);
    positions.push(bubble.x);
  }
  expect(positions[1] - positions[0]).toBeGreaterThan(2);
  expect(positions[3] - positions[2]).toBeGreaterThan(2);
  await assertNoOverflow(page);
});

test("list, statistics, details and settings errors offer working retries", async ({
  page,
  request,
}) => {
  const headers = await adminHeaders(request);
  await request.post("/api/links", {
    headers,
    data: { original_url: "https://example.org", short_code: "retry" },
  });
  await login(page);
  for (const [path, destination, ready] of [
    [
      "/api/links",
      "/links",
      () => expect(page.getByRole("article")).toHaveCount(1),
    ],
    [
      "/api/analytics",
      "/",
      () =>
        expect(page.getByRole("heading", { name: "访问量趋势" })).toBeVisible(),
    ],
    [
      "/api/links/stats/retry",
      "/?short_code=retry",
      () =>
        expect(page.locator(".visit-details tbody")).toContainText(
          "暂无访问记录",
        ),
    ],
    [
      "/api/admin/settings",
      "/settings",
      () => expect(page.getByLabel("站点名称", { exact: true })).toBeVisible(),
    ],
    [
      "/api/links/tags",
      "/",
      () =>
        expect(page.getByRole("heading", { name: "访问量趋势" })).toBeVisible(),
    ],
  ]) {
    let fail = true;
    await page.route(
      (url) => url.pathname === path,
      (route) => {
        if (fail) {
          return route.fulfill({
            status: 503,
            json: { detail: "可重试的错误" },
          });
        }
        return route.continue();
      },
    );
    await page.goto(destination);
    const alert = page.getByRole("alert").filter({ hasText: "可重试的错误" });
    await expect(alert).toBeVisible();
    fail = false;
    await alert.getByRole("button", { name: "重试" }).click();
    await ready();
    await expect(alert).not.toBeVisible();
    await page.unrouteAll({ behavior: "wait" });
  }
});

test("detail request failures can be retried without losing the dialog", async ({
  page,
  request,
}) => {
  await seedAnalytics(request);
  await login(page);
  let fail = true;
  await page.route("**/api/analytics/breakdown?**", async (route) => {
    if (fail) {
      fail = false;
      return route.fulfill({
        status: 500,
        json: { detail: "暂时无法加载统计" },
      });
    }
    return route.continue();
  });
  await page.getByRole("button", { name: "访问来源", exact: true }).click();
  await expect(page.getByRole("dialog").getByRole("alert")).toContainText(
    "暂时无法加载统计",
  );
  await page.getByRole("dialog").getByRole("button", { name: "重试" }).click();
  await expect(page.getByRole("dialog").locator(".detail-row")).toHaveCount(6);
});

test("late list and analytics results cannot overwrite newer filters", async ({
  page,
  request,
}) => {
  const headers = await adminHeaders(request);
  for (const code of ["alpha", "beta"])
    await request.post("/api/links", {
      headers,
      data: { original_url: "https://example.org", short_code: code },
    });
  await login(page);
  await navigate(page, "链接");
  const captured = deferred(),
    release = deferred();
  await page.route(
    (url) =>
      url.pathname === "/api/links" && url.searchParams.get("q") === "alpha",
    async (route) => {
      const response = await route.fetch();
      captured.resolve();
      await release.promise;
      await route.fulfill({ response });
    },
  );
  try {
    await page.getByRole("textbox", { name: "搜索链接" }).fill("alpha");
    await captured.promise;
    await page.getByRole("textbox", { name: "搜索链接" }).fill("beta");
    await expect(page.getByRole("article")).toHaveCount(1);
    await expect(page.getByRole("article")).toContainText("beta");
    const delivered = page.waitForResponse(
      (r) => new URL(r.url()).searchParams.get("q") === "alpha",
    );
    release.resolve();
    await delivered;
    await expect(page.getByRole("article")).toContainText("beta");
  } finally {
    release.resolve();
  }
  await navigate(page, "首页");
  await expect(page.getByRole("heading", { name: "访问量趋势" })).toBeVisible();
  const capturedStats = deferred(),
    releaseStats = deferred();
  await page.route(
    (url) =>
      url.pathname === "/api/analytics" &&
      url.searchParams.get("start_date") === rangeDate(6),
    async (route) => {
      const response = await route.fetch();
      capturedStats.resolve();
      await releaseStats.promise;
      await route.fulfill({ response });
    },
  );
  try {
    await selectRange(page, 7);
    await capturedStats.promise;
    await selectRange(page, 30);
    await expect(page.locator(".trend-chart circle")).toHaveCount(30);
    const delivered = page.waitForResponse(
      (r) =>
        new URL(r.url()).pathname === "/api/analytics" &&
        new URL(r.url()).searchParams.get("start_date") === rangeDate(6),
    );
    releaseStats.resolve();
    await delivered;
    await expect(page.locator(".trend-chart circle")).toHaveCount(30);
  } finally {
    releaseStats.resolve();
  }
});

test("invalid backups, size limits and failed restores preserve data and can retry", async ({
  page,
  request,
}) => {
  await login(page);
  await navigate(page, "设置");
  await page.getByLabel("站点名称", { exact: true }).fill("保留的名称");
  await page.getByLabel("随机短码长度", { exact: true }).fill("8");
  await page
    .getByLabel("公开访问地址", { exact: true })
    .fill("https://example.org/not-an-origin");
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page.getByRole("status")).toContainText("公开地址只包含");
  await expect(page).toHaveTitle("设置 · Link WithU");
  await page
    .getByLabel("公开访问地址", { exact: true })
    .fill("http://127.0.0.1:5202");
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(page).toHaveTitle("设置 · 保留的名称");
  const headers = await adminHeaders(request);
  let failBackup = true;
  await page.route("**/api/admin/backup", (route) => {
    if (failBackup) {
      failBackup = false;
      return route.fulfill({ status: 500, json: { detail: "暂时无法导出" } });
    }
    return route.continue();
  });
  await page.getByRole("button", { name: "导出全部数据", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("暂时无法导出");
  const pendingDownload = page.waitForEvent("download");
  await page.getByRole("button", { name: "导出全部数据", exact: true }).click();
  expect((await pendingDownload).suggestedFilename()).toMatch(
    /^link-withu-.*\.json$/,
  );
  const created = await (
    await request.post("/api/links", {
      headers,
      data: { original_url: "example.org" },
    })
  ).json();
  expect(created.short_code).toHaveLength(8);
  const input = page.getByLabel("选择备份文件");
  for (const content of [
    "not json",
    "{}",
    '{"format":"link-withu-backup","version":2,"links":[],"visits":[]}',
  ]) {
    await input.setInputFiles({
      name: "invalid.json",
      mimeType: "application/json",
      buffer: Buffer.from(content),
    });
    await expect(page.getByRole("status")).toContainText("请选择有效的");
    await expect(page.getByRole("dialog")).not.toBeVisible();
  }
  await input.evaluate((element) => {
    const files = new DataTransfer();
    files.items.add(
      new File([new Uint8Array(50 * 1024 * 1024 + 1)], "oversized.json", {
        type: "application/json",
      }),
    );
    element.files = files.files;
    element.dispatchEvent(new Event("change", { bubbles: true }));
  });
  await expect(page.getByRole("status")).toContainText("不能超过 50 MiB");
  const snapshot = await (
    await request.get("/api/admin/backup", { headers })
  ).json();
  const file = {
    name: "backup.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(snapshot)),
  };
  await input.setInputFiles(file);
  await page.getByLabel("输入 RESTORE 确认覆盖").fill("restore");
  await expect(page.getByRole("button", { name: "覆盖并恢复" })).toBeDisabled();
  await page.getByRole("button", { name: "取消", exact: true }).click();
  await input.setInputFiles(file);
  let fail = true;
  await page.route("**/api/admin/restore", (route) => {
    if (fail) {
      fail = false;
      return route.fulfill({ status: 400, json: { detail: "备份校验失败" } });
    }
    return route.continue();
  });
  await page.getByLabel("输入 RESTORE 确认覆盖").fill("RESTORE");
  await page.getByRole("button", { name: "覆盖并恢复" }).click();
  await expect(page.getByRole("status")).toContainText("备份校验失败");
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(
    (await (await request.get("/api/links", { headers })).json()).total,
  ).toBe(1);
  await page.getByRole("button", { name: "覆盖并恢复" }).click();
  await expect(page.getByRole("dialog")).not.toBeVisible();
  await expect(page).toHaveTitle("设置 · 保留的名称");
});

test("expired sessions, invalid storage, safe redirects and system theme", async ({
  page,
}) => {
  await page.route("**/api/site", (route) =>
    route.fulfill({
      status: 503,
      json: { detail: "temporary branding failure" },
    }),
  );
  await page.goto("/login");
  await expect(page.locator(".login-brand")).toHaveText("Link WithU");
  await page
    .getByLabel("管理员密码", { exact: true })
    .fill("browser-test-only");
  await page.getByRole("button", { name: "显示密码" }).click();
  await expect(page.getByLabel("管理员密码", { exact: true })).toHaveAttribute(
    "type",
    "text",
  );
  await page.getByRole("button", { name: "隐藏密码" }).click();
  await expect(page.getByLabel("管理员密码", { exact: true })).toHaveAttribute(
    "type",
    "password",
  );
  await page.getByRole("button", { name: "登录工作空间" }).click();
  await navigate(page, "设置");
  await page.getByRole("button", { name: "跟随系统", exact: true }).click();
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.reload();
  await expect(
    page.getByRole("button", { name: "跟随系统", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.evaluate(() =>
    localStorage.setItem(
      "link-withu-admin-session",
      JSON.stringify({ token: "expired", expires_at: 1 }),
    ),
  );
  await page.goto("/links");
  await expect(page).toHaveURL(/\/login\?redirect=/);
  await page.evaluate(() =>
    localStorage.setItem("link-withu-admin-session", "invalid-json"),
  );
  await page.reload();
  await expect(page.getByRole("heading", { name: "管理员登录" })).toBeVisible();
  for (const redirect of ["https://evil.example/", "//evil.example/"]) {
    await page.goto(`/login?redirect=${encodeURIComponent(redirect)}`);
    await page
      .getByLabel("管理员密码", { exact: true })
      .fill("browser-test-only");
    await page.getByRole("button", { name: "登录工作空间" }).click();
    await expect(page).toHaveURL(/\/$/);
    await page.evaluate(() =>
      localStorage.removeItem("link-withu-admin-session"),
    );
  }
  await login(page);
  await page.goto("/analytics?tag=missing#trend");
  await expect(page).toHaveURL("/?tag=missing#trend");
  await page.evaluate(() => {
    history.pushState(history.state, "", "/unknown-page");
    window.dispatchEvent(
      new PopStateEvent("popstate", { state: history.state }),
    );
  });
  await expect(page).toHaveURL("/");
  let fail = true;
  await page.route(
    (url) => url.pathname === "/api/links",
    (route) => {
      if (fail) {
        fail = false;
        return route.fulfill({ status: 401, json: { detail: "会话失效" } });
      }
      return route.continue();
    },
  );
  await navigate(page, "链接");
  await expect(page).toHaveURL(/\/login\?redirect=/);
  expect(
    await page.evaluate(() => localStorage.getItem("link-withu-admin-session")),
  ).toBeNull();
});
