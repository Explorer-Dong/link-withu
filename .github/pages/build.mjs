import { spawnSync } from "node:child_process";
import { cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import MarkdownIt from "markdown-it";

const root = resolve(fileURLToPath(new URL("../..", import.meta.url)));
const [binary, destination] = process.argv.slice(2);
if (!binary || !destination) {
  throw new Error(
    "Usage: node .github/pages/build.mjs <binary> <output-directory>",
  );
}
const output = resolve(destination);
const result = spawnSync(resolve(binary), ["--openapi"], { encoding: "utf8" });
if (result.status !== 0) {
  throw new Error(`Cannot generate OpenAPI: ${result.stderr || result.error}`);
}
const spec = JSON.parse(result.stdout);
if (!spec.openapi || !spec.paths || !spec.info?.version) {
  throw new Error("Invalid OpenAPI document");
}
const markdown = new MarkdownIt({ html: true, linkify: true });
const readme = await readFile(join(root, "README.md"), "utf8");
const html = markdown
  .render(readme)
  .replaceAll('src=".github/assets/', 'src="assets/')
  .replaceAll(
    'href="./AGENTS.md"',
    'href="https://github.com/Explorer-Dong/link-withu/blob/main/AGENTS.md"',
  )
  .replaceAll(
    'href="./LICENSE"',
    'href="https://github.com/Explorer-Dong/link-withu/blob/main/LICENSE"',
  );
const nav = (prefix, active) =>
  `<nav class="pages-nav" aria-label="文档导航"><a ${active === "readme" ? 'aria-current="page"' : ""} href="${prefix}">README</a><a ${active === "docs" ? 'aria-current="page"' : ""} href="${prefix}docs/">API</a></nav>`;
const head = (title, css) =>
  `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title}</title><link rel="stylesheet" href="${css}"></head><body>`;
const readmePage = `${head("Link WithU", "style.css")}<div class="pages-layout">${nav("./", "readme")}<main class="pages-readme">${html}</main></div></body></html>`;
const docsPage = `${head("Link WithU API", "../style.css")}<div class="pages-api-layout">${nav("../", "docs")}<main id="api-reference">正在加载接口文档…</main></div><script type="module">try { const { createApiReference } = await import('https://cdn.jsdelivr.net/npm/@scalar/api-reference@1.75.1/esm.js'); createApiReference('#api-reference', { url: '../openapi.json', withDefaultFonts: false, persistAuth: false, hideTestRequestButton: true, showDeveloperTools: 'never', agent: { disabled: true } }); } catch { document.querySelector('#api-reference').textContent = '接口文档加载失败，请检查网络；接口定义可通过 openapi.json 获取。'; }</script></body></html>`;
await mkdir(join(output, "docs"), { recursive: true });
await cp(join(root, ".github/assets"), join(output, "assets"), {
  recursive: true,
});
await cp(join(root, ".github/pages/style.css"), join(output, "style.css"));
await Promise.all([
  writeFile(join(output, "index.html"), readmePage),
  writeFile(join(output, "docs/index.html"), docsPage),
  writeFile(join(output, "openapi.json"), JSON.stringify(spec)),
]);
