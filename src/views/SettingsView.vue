<template>
  <div class="flex h-full flex-col">
    <header class="border-b border-slate-200 bg-white px-6 py-4">
      <h1 class="page-title">设置</h1>
      <p class="mt-0.5 text-sm text-slate-500">所有设置和学习数据仅保存在本机。</p>
    </header>
    <div class="flex min-h-0 flex-1">
      <nav class="w-44 shrink-0 space-y-0.5 border-r border-slate-200 bg-white p-2" aria-label="设置分类">
        <button
          v-for="section in sections"
          :key="section.id"
          class="block w-full rounded-lg px-3 py-2 text-left text-sm"
          :class="active === section.id ? 'bg-indigo-50 font-medium text-indigo-700' : 'text-slate-600 hover:bg-slate-100'"
          @click="active = section.id"
        >
          {{ section.label }}
        </button>
      </nav>
      <div class="min-w-0 flex-1 overflow-y-auto p-6">
        <div class="mx-auto max-w-3xl space-y-5">
          <template v-if="active === 'general'">
            <section class="card p-5">
              <h2 class="section-title">启动</h2>
              <label class="mt-3 flex items-start gap-3">
                <input v-model="form.restore_last_location" type="checkbox" class="mt-1 size-4 accent-indigo-600" @change="save" />
                <span>
                  <span class="block text-sm font-medium text-slate-800">启动时恢复上次的学习位置</span>
                  <span class="block text-xs text-slate-500">重新打开 ExamOps 时自动回到上次所在的课程页面。</span>
                </span>
              </label>
            </section>
          </template>

          <template v-else-if="active === 'data'">
            <section class="card p-5">
              <h2 class="section-title">本地数据</h2>
              <dl v-if="info" class="mt-3 grid grid-cols-[8rem_minmax(0,1fr)] gap-y-2 text-sm">
                <dt class="text-slate-500">数据目录</dt>
                <dd class="font-mono text-xs break-all text-slate-700">{{ info.data_dir }}</dd>
                <dt class="text-slate-500">数据库</dt>
                <dd class="text-slate-700">{{ formatBytes(info.database_bytes) }}（结构版本 {{ info.schema_version }}）</dd>
                <dt class="text-slate-500">资料副本</dt>
                <dd class="text-slate-700">{{ formatBytes(info.documents_bytes) }}</dd>
              </dl>
              <p class="hint mt-3">备份时关闭 ExamOps 后复制整个数据目录即可。卸载时可选择是否删除该目录（默认保留）。</p>
              <button class="btn btn-secondary mt-4" @click="openDir">打开数据目录</button>
            </section>
          </template>

          <template v-else-if="active === 'about'">
            <section class="card p-5">
              <h2 class="section-title">关于 ExamOps</h2>
              <p class="mt-2 text-sm text-slate-600">基于个人学习资料的 AI 智能复习桌面软件。单用户、本地运行，无账号、无云端存储。</p>
              <dl v-if="info" class="mt-3 grid grid-cols-[8rem_minmax(0,1fr)] gap-y-2 text-sm">
                <dt class="text-slate-500">版本</dt>
                <dd class="text-slate-700">{{ info.version }}</dd>
                <dt class="text-slate-500">平台</dt>
                <dd class="text-slate-700">{{ info.platform }}</dd>
              </dl>
            </section>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useAppStore } from '@/stores/app'
import { useToastStore } from '@/stores/toast'
import { AppService } from '@/services/app'
import type { AppInfo, AppSettings } from '@/types/app'
import { formatBytes } from '@/utils/format'

const app = useAppStore()
const toast = useToastStore()
const sections = [
  { id: 'general', label: '通用' },
  { id: 'data', label: '数据与存储' },
  { id: 'about', label: '关于' }
] as const
const active = ref<(typeof sections)[number]['id']>('general')
const info = ref<AppInfo | null>(null)
const form = reactive<AppSettings>({ ...app.settings })

onMounted(async () => {
  if (!app.ready) await app.load().catch(() => undefined)
  Object.assign(form, app.settings)
  info.value = await AppService.info().catch((e) => { toast.error(e); return null })
})

async function save() {
  try {
    Object.assign(form, await app.save({ ...form }))
    toast.success('设置已保存')
  } catch (e) {
    toast.error(e)
  }
}

async function openDir() {
  try { await AppService.openDataDir() } catch (e) { toast.error(e) }
}
</script>
