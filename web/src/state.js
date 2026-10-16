import { ref } from "vue";
import { version } from "../package.json";

const SESSION_KEY = "link-withu-admin-session";
function readSession() {
  try {
    const value = JSON.parse(localStorage.getItem(SESSION_KEY) || "null");
    return value?.token && value.expires_at > Date.now() / 1000 ? value : null;
  } catch {
    return null;
  }
}
export const session = ref(readSession());
export const site = ref({ app_name: "Link WithU", version });
export const tags = ref([]);
export const toast = ref(null);
let toastTimer;
export function notify(message, error = false) {
  toast.value = { message, error };
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toast.value = null;
  }, 4500);
}
export function saveSession(value) {
  session.value = value;
  if (value) localStorage.setItem(SESSION_KEY, JSON.stringify(value));
  else localStorage.removeItem(SESSION_KEY);
}
export function isLoggedIn() {
  return !!session.value?.token && session.value.expires_at > Date.now() / 1000;
}
export const theme = ref(localStorage.getItem("theme") || "system");
export function applyTheme() {
  const dark =
    theme.value === "dark" ||
    (theme.value === "system" &&
      matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  localStorage.setItem("theme", theme.value);
}
export function formatDate(value) {
  if (!value) return "—";
  return new Date(
    /Z$|[+-]\d\d:\d\d$/.test(value) ? value : value + "Z",
  ).toLocaleString("zh-CN", { hour12: false });
}
export const number = (value) =>
  new Intl.NumberFormat("zh-CN").format(value || 0);
export async function copy(value) {
  try {
    await navigator.clipboard.writeText(value);
    notify("已复制到剪贴板");
  } catch {
    notify("复制失败，请手动复制链接", true);
  }
}
