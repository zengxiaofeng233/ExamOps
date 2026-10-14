import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'home',
    component: () => import('@/views/HomeView.vue')
  },
  {
    path: '/courses',
    name: 'courses',
    component: () => import('@/views/CoursesView.vue')
  },
  {
    path: '/courses/:id(\\d+)',
    component: () => import('@/views/course/CourseLayout.vue'),
    props: route => ({ courseId: Number(route.params.id) }),
    children: [
      {
        path: '',
        name: 'course-overview',
        component: () => import('@/views/course/OverviewView.vue'),
        meta: { tab: '概览' }
      },
      {
        path: 'documents',
        name: 'documents',
        component: () => import('@/views/course/DocumentsView.vue'),
        meta: { tab: '资料' }
      },
      {
        path: 'tutor',
        name: 'tutor',
        component: () => import('@/views/course/TutorView.vue'),
        meta: { tab: 'AI 问答' }
      }
    ]
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('@/views/SettingsView.vue')
  },
  {
    path: '/:pathMatch(.*)*',
    redirect: '/'
  }
]

const router = createRouter({
  history: createWebHistory(),
  routes
})

export default router
