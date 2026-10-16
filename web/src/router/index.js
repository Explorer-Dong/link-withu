import { createRouter, createWebHistory } from "vue-router";
import { isLoggedIn, saveSession } from "../state";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/login",
      component: () => import("../views/Login.vue"),
      meta: { title: "登录", public: true },
    },
    {
      path: "/",
      component: () => import("../views/Analytics.vue"),
      meta: { title: "访问统计" },
    },
    {
      path: "/links",
      component: () => import("../views/Home.vue"),
      meta: { title: "链接管理" },
    },
    {
      path: "/settings",
      component: () => import("../views/Settings.vue"),
      meta: { title: "设置" },
    },
    {
      path: "/analytics",
      redirect: (to) => ({ path: "/", query: to.query, hash: to.hash }),
    },
    { path: "/:pathMatch(.*)*", redirect: "/" },
  ],
  scrollBehavior: () => ({ top: 0 }),
});
router.beforeEach((to) => {
  if (!to.meta.public && !isLoggedIn()) {
    saveSession(null);
    return { path: "/login", query: { redirect: to.fullPath } };
  }
  if (to.path === "/login" && isLoggedIn()) return "/";
  document.title = `${to.meta.title || "Link WithU"} · Link WithU`;
});
window.addEventListener("auth-expired", () => {
  const redirect = router.currentRoute.value.fullPath;
  if (router.currentRoute.value.path !== "/login")
    router.replace({ path: "/login", query: { redirect } });
});
export default router;
