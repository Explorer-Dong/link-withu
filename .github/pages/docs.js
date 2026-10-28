const form = document.querySelector(".pages-server");
const input = document.querySelector("#deployment-url");
const storageKey = "link-withu-documentation-base-url";
const configuration = {
  url: "../openapi.json",
  servers: [{ url: "https://go.example.com" }],
  withDefaultFonts: false,
  persistAuth: false,
  hideTestRequestButton: true,
  showDeveloperTools: "never",
  agent: { disabled: true },
};
function normalize(value) {
  const url = new URL(value);
  if (
    !["http:", "https:"].includes(url.protocol) ||
    url.username ||
    url.password ||
    url.pathname !== "/" ||
    url.search ||
    url.hash
  ) {
    throw new Error("请输入只包含协议、域名和可选端口的部署地址");
  }
  return url.origin;
}
try {
  const saved = localStorage.getItem(storageKey);
  if (saved) configuration.servers[0].url = normalize(saved);
} catch {
  // The page remains usable when storage is unavailable or contains invalid data.
}
input.value = configuration.servers[0].url;
let reference;
let createReference;
form.addEventListener("submit", (event) => {
  event.preventDefault();
  try {
    const url = normalize(input.value);
    input.setCustomValidity("");
    input.value = url;
    configuration.servers = [{ url }];
    if (reference) {
      reference.destroy();
      reference = createReference("#api-reference", configuration);
    }
    try {
      localStorage.setItem(storageKey, url);
    } catch {
      /* Storage is optional. */
    }
  } catch (error) {
    input.setCustomValidity(error.message);
    input.reportValidity();
  }
});
input.addEventListener("input", () => input.setCustomValidity(""));
try {
  ({ createApiReference: createReference } =
    await import("https://cdn.jsdelivr.net/npm/@scalar/api-reference@1.75.1/esm.js"));
  reference = createReference("#api-reference", configuration);
} catch {
  document.querySelector("#api-reference").textContent =
    "接口文档加载失败，请检查网络；接口定义可通过 openapi.json 获取。";
}
