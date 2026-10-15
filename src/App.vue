<template>
  <StartupSplash v-if="showSplash" @finished="showSplash = false" />
  <div :inert="showSplash" class="flex h-full w-full overflow-hidden">
    <AppSidebar />
    <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
      <UpdateNotice v-if="!showSplash" />
      <div class="min-h-0 flex-1 overflow-hidden">
        <RouterView />
      </div>
    </main>
    <ToastHost />
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import AppSidebar from '@/components/layout/AppSidebar.vue'
import ToastHost from '@/components/ui/ToastHost.vue'
import StartupSplash from '@/components/layout/StartupSplash.vue'
import UpdateNotice from '@/components/layout/UpdateNotice.vue'
import { useUpdateStore } from '@/stores/updates'
import { useAppStore } from '@/stores/app'
import { useCourseStore } from '@/stores/course'
import { AppService } from '@/services/app'

const router = useRouter()
const app = useAppStore()
const courses = useCourseStore()
const updates = useUpdateStore()
const showSplash = ref(!window.matchMedia('(prefers-reduced-motion: reduce)').matches)

router.afterEach((to) => app.rememberLocation(to.fullPath))

onMounted(async () => {
  void updates.check()
  await Promise.allSettled([app.load(), courses.ensureLoaded()])
  // Resume where the user stopped last time (roadmap: Study State Engine).
  if (!app.settings.restore_last_location) return
  await router.isReady()
  if (router.currentRoute.value.path !== '/') return
  try {
    const last = await AppService.lastLocation()
    if (last?.path && last.path !== '/') await router.replace(last.path)
  } catch {
    // A broken saved location must never block start-up.
  }
})
</script>
