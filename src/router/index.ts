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
        path: 'knowledge',
        name: 'knowledge',
        component: () => import('@/views/course/KnowledgeView.vue'),
        meta: { tab: '知识点' }
      },
      {
        path: 'paths',
        name: 'paths',
        component: () => import('@/views/course/PathsView.vue'),
        meta: { tab: '学习路径' }
      },
      {
        path: 'learn',
        name: 'learn',
        component: () => import('@/views/course/LearnView.vue'),
        meta: { tab: 'AI 学习' }
      },
      {
        path: 'quiz',
        name: 'quiz',
        component: () => import('@/views/course/QuizView.vue'),
        meta: { tab: '测验' }
      },
      {
        path: 'review',
        name: 'review',
        component: () => import('@/views/course/ReviewView.vue'),
        meta: { tab: '复习' }
      },
      {
        path: 'tutor',
        name: 'tutor',
        component: () => import('@/views/course/TutorView.vue'),
        meta: { tab: 'AI 问答' }
      },
      {
        path: 'videos',
        name: 'videos',
        component: () => import('@/views/course/VideosView.vue'),
        meta: { tab: '网课' }
      },
      {
        path: 'notes',
        name: 'notes',
        component: () => import('@/views/course/NotesView.vue'),
        meta: { tab: '笔记' }
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
