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
const tokens = markdown.parse(readme, {});
const headings = [];
const usedIds = new Set();
for (let index = 0; index < tokens.length; index += 1) {
  const token = tokens[index];
  if (token.type !== "heading_open" || !["h2", "h3"].includes(token.tag))
    continue;
  const title = (tokens[index + 1].children || [])
    .filter((child) => ["text", "code_inline"].includes(child.type))
    .map((child) => child.content)
    .join("")
    .trim();
  const base =
    title
      .toLowerCase()
      .replace(/[^\p{L}\p{N}\s_-]/gu, "")
      .trim()
      .replace(/\s+/g, "-") || "section";
  let id = base;
  let count = 1;
  while (usedIds.has(id)) id = `${base}-${count++}`;
  usedIds.add(id);
  token.attrSet("id", id);
  headings.push({ title, id, level: token.tag });
}
const toc = `<details class="pages-toc" open><summary>目录</summary><ul>${headings.map(({ title, id, level }) => `<li class="${level}"><a href="#${encodeURIComponent(id)}">${markdown.utils.escapeHtml(title)}</a></li>`).join("")}</ul></details>`;
const html = markdown.renderer
  .render(tokens, markdown.options, {})
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
  `<nav class="pages-nav" aria-label="文档导航"><a ${active === "readme" ? 'aria-current="page"' : ""} href="${prefix}">README</a><a ${active === "docs" ? 'aria-current="page"' : ""} href="${prefix}docs/">API</a>${active === "readme" ? toc : ""}</nav>`;
const head = (title, css, icon) =>
  `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title}</title><link rel="icon" type="image/svg+xml" href="${icon}"><link rel="stylesheet" href="${css}"></head><body>`;
const readmePage = `${head("Link WithU", "style.css", "favicon.svg")}<div class="pages-layout">${nav("./", "readme")}<main class="pages-readme">${html}</main></div><script>if (matchMedia('(max-width: 720px)').matches) document.querySelector('.pages-toc').removeAttribute('open');</script></body></html>`;
const docsPage = `${head("Link WithU API", "../style.css", "../favicon.svg")}<div class="pages-api-layout">${nav("../", "docs")}<main class="pages-docs"><form class="pages-server"><label for="deployment-url">部署地址</label><input id="deployment-url" type="url" required placeholder="https://go.example.com"><button type="submit">应用地址</button></form><div id="api-reference">正在加载接口文档…</div></main></div><script type="module" src="../docs.js"></script></body></html>`;
await mkdir(join(output, "docs"), { recursive: true });
await cp(join(root, ".github/assets"), join(output, "assets"), {
  recursive: true,
});
await cp(join(root, ".github/pages/style.css"), join(output, "style.css"));
await cp(join(root, ".github/pages/docs.js"), join(output, "docs.js"));
await cp(join(root, "web/public/favicon.svg"), join(output, "favicon.svg"));
await Promise.all([
  writeFile(join(output, "index.html"), readmePage),
  writeFile(join(output, "docs/index.html"), docsPage),
  writeFile(join(output, "openapi.json"), JSON.stringify(spec)),
]);
