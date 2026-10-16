import { session, saveSession } from "../state";

export async function request(path, options = {}) {
  const headers = new Headers(options.headers);
  if (session.value?.token)
    headers.set("Authorization", `Bearer ${session.value.token}`);
  if (
    options.body &&
    typeof options.body === "string" &&
    !headers.has("Content-Type")
  )
    headers.set("Content-Type", "application/json");
  const res = await fetch(path, { ...options, headers });
  if (!res.ok) {
    if (res.status === 401 && !path.endsWith("/login")) {
      saveSession(null);
      window.dispatchEvent(new Event("auth-expired"));
    }
    const data = await res.json().catch(() => ({}));
    const detail = Array.isArray(data.detail)
      ? data.detail.map((item) => item.msg).join("；")
      : data.detail;
    throw new Error(detail || `请求失败（${res.status}）`);
  }
  return res;
}
export async function api(path, options) {
  const res = await request(path, options);
  return res.status === 204 ? null : res.json();
}
export const adminLogin = (password) =>
  api("/api/admin/login", {
    method: "POST",
    body: JSON.stringify({ password }),
  });
export const listLinks = (params) =>
  api(`/api/links?${new URLSearchParams(params)}`);
export const createLink = (body) =>
  api("/api/links", { method: "POST", body: JSON.stringify(body) });
export const updateLink = (code, body) =>
  api(`/api/links/${encodeURIComponent(code)}`, {
    method: "PUT",
    body: JSON.stringify(body),
  });
export const deleteLink = (code) =>
  api(`/api/links/${encodeURIComponent(code)}`, { method: "DELETE" });
export const getStats = (code, params = {}) =>
  api(
    `/api/links/stats/${encodeURIComponent(code)}?${new URLSearchParams(params)}`,
  );
export async function download(path) {
  const res = await request(path);
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download =
    res.headers.get("Content-Disposition")?.match(/filename="([^"]+)"/)?.[1] ||
    "link-withu-export";
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
