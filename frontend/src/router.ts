import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import { useAuthStore } from './stores/auth'

const routes: RouteRecordRaw[] = [
  {
    path: '/login',
    name: 'login',
    component: () => import('./views/LoginView.vue'),
  },
  {
    path: '/',
    name: 'home',
    component: () => import('./views/ChatView.vue'),
  },
  {
    path: '/c/:id',
    name: 'chat',
    component: () => import('./views/ChatView.vue'),
    props: true,
  },
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
})

router.beforeEach(async (to) => {
  if (to.name === 'login') return true

  const auth = useAuthStore()
  if (!auth.checked) {
    await auth.fetchMe()
  }
  if (!auth.user) {
    return { name: 'login', query: { from: to.fullPath } }
  }
  return true
})
