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
          <ModelSettings v-if="active === 'models'" />

          <template v-else-if="active === 'general'">
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

          <template v-else-if="active === 'study'">
            <section class="card p-5">
              <h2 class="section-title">小测</h2>
              <p class="mt-0.5 text-sm text-slate-500">小测结果会计入知识点掌握度，并据此安排复习。</p>
              <fieldset class="mt-4">
                <legend class="label">什么时候建议做小测</legend>
                <div class="space-y-1.5">
                  <label v-for="(label, value) in TRIGGER_LABELS" :key="value" class="flex items-center gap-2 text-sm text-slate-700">
                    <input v-model="form.quiz.trigger" type="radio" :value="value" class="accent-indigo-600" @change="save" />{{ label }}
                  </label>
                </div>
                <p class="hint">“从零学习”时按这里的设置提示你做小测；任何时候都可以在“测验”页手动开始。</p>
              </fieldset>
              <div class="mt-4 grid gap-4 sm:grid-cols-2">
                <div>
                  <label class="label" for="quiz-count-setting">每次题目数量</label>
                  <input id="quiz-count-setting" v-model.number="form.quiz.question_count" type="number" min="1" max="20" step="1" class="input" @change="save" />
                </div>
                <label class="flex items-center gap-2 self-end pb-2 text-sm text-slate-700">
                  <input v-model="form.quiz.allow_skip" type="checkbox" class="accent-indigo-600" @change="save" />允许跳过题目（跳过按答错计）
                </label>
              </div>
              <fieldset class="mt-4">
                <legend class="label">默认题型</legend>
                <div class="grid gap-1.5 sm:grid-cols-2">
                  <label v-for="(label, kind) in QUESTION_KIND_LABELS" :key="kind" class="flex items-start gap-2 text-sm text-slate-700">
                    <input v-model="form.quiz.kinds" type="checkbox" :value="kind" class="mt-1 accent-indigo-600" :disabled="form.quiz.kinds.length === 1 && form.quiz.kinds.includes(kind)" @change="save" />
                    <span>{{ label }}<span class="block text-xs text-slate-500">{{ QUESTION_KIND_HINTS[kind] }}</span></span>
                  </label>
                </div>
              </fieldset>
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
import { onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import ModelSettings from '@/components/settings/ModelSettings.vue'
import { useAppStore } from '@/stores/app'
import { useToastStore } from '@/stores/toast'
import { AppService } from '@/services/app'
import type { AppInfo, AppSettings } from '@/types/app'
import { QUESTION_KIND_HINTS, QUESTION_KIND_LABELS, TRIGGER_LABELS } from '@/utils/assessment'
import { formatBytes } from '@/utils/format'

const app = useAppStore()
const toast = useToastStore()
const sections = [
  { id: 'models', label: 'AI 模型' },
  { id: 'general', label: '通用' },
  { id: 'study', label: '学习与测验' },
  { id: 'data', label: '数据与存储' },
  { id: 'about', label: '关于' }
] as const
type SectionId = (typeof sections)[number]['id']
const route = useRoute()
const router = useRouter()
const active = ref<SectionId>(sections.some(s => s.id === route.query.section) ? (route.query.section as SectionId) : 'models')
watch(active, (section) => router.replace({ query: { section } }))
const info = ref<AppInfo | null>(null)
/** Plain copy (reactive proxies cannot be structured-cloned). */
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value))
const form = reactive<AppSettings>(clone(app.settings))

onMounted(async () => {
  if (!app.ready) await app.load().catch(() => undefined)
  Object.assign(form, clone(app.settings))
  info.value = await AppService.info().catch((e) => { toast.error(e); return null })
})

async function save() {
  try {
    // Only this page's fields; the model page saves the context settings itself.
    const next = { ...clone(app.settings), restore_last_location: form.restore_last_location, quiz: clone(form.quiz) }
    Object.assign(form, clone(await app.save(next)))
    toast.success('设置已保存')
  } catch (e) {
    toast.error(e)
  }
}

async function openDir() {
  try { await AppService.openDataDir() } catch (e) { toast.error(e) }
}
</script>
