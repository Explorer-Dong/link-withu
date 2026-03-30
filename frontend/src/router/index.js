import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'home', component: () => import('../views/Home.vue'), meta: { title: '短链服务' } },
  ],
})

router.beforeEach((to) => {
  if (to.meta?.title) document.title = to.meta.title
})

export default router
