import { expect } from "@playwright/test";
import { test } from "./helpers";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
let directory;
let server;
let base;
let version;

test.beforeAll(async () => {
  directory = await mkdtemp(join(tmpdir(), "link-withu-pages-"));
  version = JSON.parse(await readFile(join(root, "web/package.json"))).version;
  const binary =
    process.env.LINK_WITHU_BIN || join(root, "api/target/debug/link-withu");
  const build = spawnSync(
    process.execPath,
    [join(root, ".github/pages/build.mjs"), binary, directory],
    { encoding: "utf8" },
  );
  expect(build.status, build.stderr).toBe(0);
  server = createServer(async (request, response) => {
    try {
      const path = new URL(request.url, "http://localhost").pathname;
      if (!path.startsWith("/link-withu/") || path.includes(".."))
        throw new Error("Invalid path");
      let name = path.slice("/link-withu/".length);
      if (name.endsWith("/") || !name) name += "index.html";
      const content = await readFile(join(directory, name));
      const extension = name.split(".").pop();
      response.setHeader(
        "Content-Type",
        {
          html: "text/html; charset=utf-8",
          json: "application/json",
          css: "text/css",
          png: "image/png",
        }[extension] || "application/octet-stream",
      );
      response.end(content);
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  base = `http://127.0.0.1:${server.address().port}/link-withu/`;
});

test.afterAll(async () => {
  if (server) await new Promise((done) => server.close(done));
  if (directory) await rm(directory, { recursive: true, force: true });
});

test("README and API pages render under a project subpath", async ({
  page,
  request,
}) => {
  const cdn =
    "https://cdn.jsdelivr.net/npm/@scalar/api-reference@1.75.1/esm.js";
  await page.route(cdn, (route) =>
    route.fulfill({
      contentType: "text/javascript",
      headers: { "access-control-allow-origin": "*" },
      body: `export async function createApiReference(selector, config) {
      const data = await (await fetch(config.url)).json();
      window.scalarConfig = config;
      window.document.querySelector('.pages-api-layout').insertBefore(
        window.document.createElement('div'), window.document.querySelector(selector));
      window.document.querySelector(selector).textContent = data.info.title + ' ' + data.info.version;
    }`,
    }),
  );
  await page.goto(base);
  await expect(page.getByRole("heading", { name: "Link WithU" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "快速开始" })).toBeVisible();
  await page.getByText("桌面端", { exact: true }).click();
  const light = page.locator('a[href$="#gh-light-mode-only"] img').first();
  const dark = page.locator('a[href$="#gh-dark-mode-only"] img').first();
  await expect(light).toBeVisible();
  await expect(dark).toBeHidden();
  const image = await request.get(
    new URL(await light.getAttribute("src"), base).href,
  );
  expect(image.status()).toBe(200);
  expect(image.headers()["content-type"]).toBe("image/png");
  const images = await page
    .locator('img[src^="assets/"]')
    .evaluateAll((nodes) => nodes.map((node) => node.getAttribute("src")));
  expect(images).toHaveLength(12);
  for (const path of images) {
    expect((await request.get(new URL(path, base).href)).status()).toBe(200);
  }
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(light).toBeHidden();
  await expect(dark).toBeVisible();
  await page.getByRole("link", { name: "API", exact: true }).click();
  await expect(page).toHaveURL(`${base}docs/`);
  await expect(page.locator("#api-reference")).toHaveText(
    `Link WithU ${version}`,
  );
  expect(
    await page
      .locator("#api-reference")
      .evaluate((element) => element.getBoundingClientRect().width),
  ).toBeGreaterThan(300);
  expect(
    await page.evaluate(() => window.scalarConfig.hideTestRequestButton),
  ).toBe(true);
  const spec = await (await request.get(`${base}openapi.json`)).json();
  expect(spec.paths["/api/links/by-code/{short_code}"].get).toBeTruthy();
  await page.getByRole("link", { name: "README", exact: true }).click();
  await expect(page).toHaveURL(base);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
