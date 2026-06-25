import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'home', component: () => import('../views/Home.vue'), meta: { title: 'Link WithU' } },
  ],
})

router.beforeEach((to) => {
  if (to.meta?.title) document.title = to.meta.title
})

export default router
