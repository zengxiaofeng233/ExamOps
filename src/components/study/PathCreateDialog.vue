<template>
  <AppModal :open="open" title="新建学习路径" size="lg" :persistent="busy" @close="emit('close')">
    <div class="mb-4 grid grid-cols-3 gap-1 rounded-lg bg-slate-100 p-1" role="tablist" aria-label="创建方式">
      <button
        v-for="item in ways"
        :key="item.value"
        role="tab"
        :aria-selected="way === item.value"
        class="rounded-md px-2 py-1.5 text-sm font-medium transition-colors"
        :class="way === item.value ? 'bg-surface text-indigo-700 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
        @click="way = item.value"
      >
        {{ item.label }}
      </button>
    </div>

    <form id="path-form" class="space-y-3" novalidate @submit.prevent="create">
      <div>
        <p class="label">学习模式</p>
        <div class="grid grid-cols-3 gap-2">
          <label
            v-for="item in MODES"
            :key="item"
            class="cursor-pointer rounded-lg border p-2.5 text-sm"
            :class="mode === item ? 'border-indigo-400 bg-indigo-50/60' : 'border-slate-200 hover:bg-slate-50'"
          >
            <input v-model="mode" type="radio" :value="item" class="sr-only" />
            <span class="font-medium text-slate-800">{{ MODE_LABELS[item] }}</span>
            <span class="mt-0.5 block text-xs text-slate-500">{{ MODE_HINTS[item] }}</span>
          </label>
        </div>
      </div>

      <template v-if="way === 'template'">
        <p class="text-sm text-slate-600">按资料的章节顺序自动生成步骤，例如“阅读教材 → 观看关联网课 → AI 讲解 → 小测”，生成后可以随意调整。</p>
        <div v-if="documents.length > 1">
          <p class="label">跟随哪些资料（不选则全部）</p>
          <div class="max-h-40 space-y-1 overflow-y-auto rounded-lg border border-slate-200 p-2">
            <label v-for="doc in documents" :key="doc.id" class="flex items-center gap-2 rounded px-1.5 py-1 text-sm hover:bg-slate-50">
              <input v-model="documentIds" type="checkbox" :value="doc.id" class="accent-indigo-600" />
              <span class="truncate">{{ doc.filename }}</span>
            </label>
          </div>
        </div>
        <label class="flex items-center gap-2 text-sm text-slate-700">
          <input v-model="includeQuiz" type="checkbox" class="accent-indigo-600" />每章结束后加入一次小测
        </label>
        <div>
          <label class="label" for="path-name-template">名称（可选）</label>
          <input id="path-name-template" v-model="name" class="input" maxlength="40" placeholder="默认按模式命名" />
        </div>
      </template>

      <template v-else-if="way === 'ai'">
        <p class="text-sm text-slate-600">AI 会根据你的资料目录、网课、知识点学习状态和考试日期，推荐一条学习路径。推荐的每一步都会校验，只保留能对应到你资料的步骤。</p>
        <div>
          <label class="label" for="path-preferences">你的要求（可选）</label>
          <textarea
            id="path-preferences"
            v-model="preferences"
            class="input"
            rows="3"
            maxlength="500"
            placeholder="例如：每天学习 1 小时，两周内学完前三章；第二章是重点；不看网课"
          />
        </div>
        <p v-if="!models.configured && models.loaded" class="text-sm text-amber-700">
          尚未配置 AI 模型。<RouterLink to="/settings?section=models" class="underline">前往设置</RouterLink>
        </p>
      </template>

      <template v-else>
        <div>
          <label class="label" for="path-name">名称</label>
          <input id="path-name" v-model="name" class="input" maxlength="40" placeholder="例如：期中复习计划" />
        </div>
        <div>
          <label class="label" for="path-description">说明（可选）</label>
          <textarea id="path-description" v-model="description" class="input" rows="2" maxlength="200" />
        </div>
        <p class="text-sm text-slate-500">创建后逐个添加步骤，可以调整顺序、修改或删除。</p>
      </template>
      <p v-if="busy && way === 'ai'" class="flex items-center gap-2 text-sm text-indigo-700" role="status">
        <ArrowPathIcon class="size-4 animate-spin" />AI 正在规划学习路径…
      </p>
    </form>
    <template #footer>
      <button class="btn btn-secondary" :disabled="busy" @click="emit('close')">取消</button>
      <button class="btn btn-primary" type="submit" form="path-form" :disabled="!canCreate">{{ busy ? '创建中…' : '创建' }}</button>
    </template>
  </AppModal>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowPathIcon } from '@heroicons/vue/20/solid'
import AppModal from '@/components/ui/AppModal.vue'
import { PathService } from '@/services/study'
import { useModelStore } from '@/stores/models'
import { useToastStore } from '@/stores/toast'
import type { Document } from '@/types/document'
import type { PathMode } from '@/types/study'
import { MODE_LABELS } from '@/utils/study'

const MODES: PathMode[] = ['learn', 'review', 'sprint']
const MODE_HINTS: Record<PathMode, string> = {
  learn: '循序渐进学新内容',
  review: '回忆 + 小测巩固',
  sprint: '时间紧，抓重点'
}

const props = defineProps<{ open: boolean; courseId: number; documents: Document[] }>()
const emit = defineEmits<{ close: []; created: [pathId: number] }>()
const models = useModelStore()
const toast = useToastStore()

const ways = [
  { value: 'template' as const, label: '按章节生成' },
  { value: 'ai' as const, label: 'AI 推荐' },
  { value: 'blank' as const, label: '空白路径' }
]
const way = ref<'template' | 'ai' | 'blank'>('template')
const mode = ref<PathMode>('learn')
const name = ref('')
const description = ref('')
const preferences = ref('')
const documentIds = ref<number[]>([])
const includeQuiz = ref(true)
const busy = ref(false)

const canCreate = computed(() => {
  if (busy.value) return false
  if (way.value === 'blank') return !!name.value.trim()
  if (way.value === 'ai') return models.configured
  return true
})

watch(() => props.open, (open) => {
  if (!open) return
  name.value = ''
  description.value = ''
  preferences.value = ''
  documentIds.value = []
  models.load().catch(() => undefined)
})

async function create() {
  if (!canCreate.value) return
  busy.value = true
  try {
    if (way.value === 'template') {
      const detail = await PathService.fromTemplate({
        course_id: props.courseId, name: name.value.trim() || null, mode: mode.value, document_ids: documentIds.value, include_quiz: includeQuiz.value
      })
      toast.success(`已生成 ${detail.steps.length} 个步骤`)
      emit('created', detail.path.id)
    } else if (way.value === 'ai') {
      const detail = await PathService.recommend({ course_id: props.courseId, mode: mode.value, preferences: preferences.value })
      toast.success(`AI 推荐了 ${detail.steps.length} 个步骤，可以继续调整`)
      emit('created', detail.path.id)
    } else {
      const path = await PathService.save({ id: null, course_id: props.courseId, name: name.value.trim(), description: description.value, mode: mode.value })
      emit('created', path.id)
    }
  } catch (e) {
    toast.error(e)
  } finally {
    busy.value = false
  }
}
</script>
