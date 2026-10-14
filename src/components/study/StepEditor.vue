<template>
  <AppModal :open="open" :title="form.id ? '编辑步骤' : '添加步骤'" size="lg" @close="emit('close')">
    <form id="step-form" class="grid gap-3 sm:grid-cols-2" novalidate @submit.prevent="save">
      <div>
        <label class="label" for="step-kind">类型</label>
        <select id="step-kind" v-model="form.kind" class="input">
          <option v-for="(label, kind) in STEP_KIND_LABELS" :key="kind" :value="kind">{{ label }}</option>
        </select>
      </div>
      <div>
        <label class="label" for="step-minutes">预计用时（分钟）</label>
        <input id="step-minutes" v-model.number="form.estimated_minutes" type="number" min="1" max="600" step="1" class="input" />
      </div>

      <template v-if="needs === 'document'">
        <div>
          <label class="label" for="step-document">资料</label>
          <select id="step-document" v-model="documentId" class="input">
            <option :value="null">选择资料</option>
            <option v-for="doc in documents" :key="doc.id" :value="doc.id">{{ doc.filename }}</option>
          </select>
        </div>
        <div>
          <label class="label" for="step-section">章节（可选）</label>
          <SectionSelect id="step-section" v-model="form.section_id" :sections="documentSections" placeholder="整份资料" />
        </div>
      </template>
      <div v-else-if="needs === 'resource'" class="sm:col-span-2">
        <label class="label" for="step-resource">网课</label>
        <select id="step-resource" v-model="form.resource_id" class="input">
          <option :value="null">选择网课</option>
          <option v-for="video in resources" :key="video.id" :value="video.id">{{ video.title }}</option>
        </select>
        <p v-if="!resources.length" class="hint">还没有网课，请先在“网课”页添加。</p>
      </div>
      <div v-else class="sm:col-span-2">
        <label class="label" for="step-section-any">章节{{ needs === 'section' ? '' : '（可选）' }}</label>
        <SectionSelect id="step-section-any" v-model="form.section_id" :sections="sections" :placeholder="needs === 'section' ? '选择章节' : '不限章节'" />
      </div>

      <div class="sm:col-span-2">
        <label class="label" for="step-title">标题（可选）</label>
        <input id="step-title" v-model="form.title" class="input" maxlength="80" :placeholder="`留空则自动生成，例如“${STEP_KIND_LABELS[form.kind]}：第二章”`" />
      </div>
      <div class="sm:col-span-2">
        <label class="label" for="step-instructions">说明（可选）</label>
        <textarea id="step-instructions" v-model="form.instructions" class="input" rows="2" maxlength="1000" placeholder="例如：重点看例题 3 和 4；把公式抄写一遍" />
      </div>
      <div v-if="candidatePoints.length" class="sm:col-span-2">
        <p class="label">涉及的知识点（可选）</p>
        <div class="max-h-36 overflow-y-auto rounded-lg border border-slate-200 p-2">
          <label v-for="point in candidatePoints" :key="point.id" class="flex items-center gap-2 rounded px-1.5 py-1 text-sm hover:bg-slate-50">
            <input v-model="form.knowledge_point_ids" type="checkbox" :value="point.id" class="accent-indigo-600" />
            <span class="truncate">{{ point.title }}</span>
          </label>
        </div>
        <p class="hint">完成阅读、讲解、网课或回忆类步骤时，这些知识点会记为“学过”；不选则按章节自动匹配。</p>
      </div>
      <p v-if="error" class="text-sm text-rose-600 sm:col-span-2" role="alert">{{ error }}</p>
    </form>
    <template #footer>
      <button class="btn btn-secondary" @click="emit('close')">取消</button>
      <button class="btn btn-primary" type="submit" form="step-form" :disabled="saving">{{ saving ? '保存中…' : '保存' }}</button>
    </template>
  </AppModal>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import AppModal from '@/components/ui/AppModal.vue'
import SectionSelect from './SectionSelect.vue'
import { PathService } from '@/services/study'
import type { CourseSection, Document } from '@/types/document'
import type { KnowledgePoint, PathStep, Resource, StepInput } from '@/types/study'
import { errorMessage } from '@/utils/errors'
import { STEP_KIND_LABELS, STEP_NEEDS } from '@/utils/study'

const props = defineProps<{
  open: boolean
  pathId: number
  step: PathStep | null
  afterStepId?: number | null
  sections: CourseSection[]
  documents: Document[]
  resources: Resource[]
  points: KnowledgePoint[]
}>()
const emit = defineEmits<{ close: []; saved: [step: PathStep] }>()

const blank = (): StepInput => ({
  id: null, path_id: props.pathId, kind: 'read_text', title: '', instructions: '', document_id: null, section_id: null,
  resource_id: null, knowledge_point_ids: [], estimated_minutes: 20, after_step_id: props.afterStepId ?? null
})
const form = ref<StepInput>(blank())
const documentId = ref<number | null>(null)
const saving = ref(false)
const error = ref('')

const needs = computed(() => STEP_NEEDS[form.value.kind])
const documentSections = computed(() => props.sections.filter(s => s.document_id === documentId.value))
const sectionById = computed(() => new Map(props.sections.map(s => [s.id, s])))

/** Points of the chosen chapter (and its sub-sections), or all points. */
const candidatePoints = computed(() => {
  const root = form.value.section_id
  if (!root) return props.points
  const inTree = (id: number | null) => {
    let current = id ? sectionById.value.get(id) : undefined
    while (current) {
      if (current.id === root) return true
      current = current.parent_id ? sectionById.value.get(current.parent_id) : undefined
    }
    return false
  }
  return props.points.filter(p => inTree(p.section_id))
})

watch(() => props.open, (open) => {
  if (!open) return
  error.value = ''
  const step = props.step
  form.value = step
    ? {
        id: step.id, path_id: step.path_id, kind: step.kind, title: step.title, instructions: step.instructions,
        document_id: step.document_id, section_id: step.section_id, resource_id: step.resource_id,
        knowledge_point_ids: [...step.knowledge_point_ids], estimated_minutes: step.estimated_minutes
      }
    : blank()
  documentId.value = step?.document_id ?? props.documents[0]?.id ?? null
})

watch(documentId, (id) => {
  // A chapter of another document no longer applies.
  if (needs.value === 'document' && form.value.section_id && sectionById.value.get(form.value.section_id)?.document_id !== id) {
    form.value.section_id = null
  }
})

async function save() {
  error.value = ''
  const input: StepInput = { ...form.value, knowledge_point_ids: form.value.knowledge_point_ids.filter(id => candidatePoints.value.some(p => p.id === id)) }
  if (needs.value === 'document') {
    if (!documentId.value) return (error.value = '请选择资料')
    input.document_id = documentId.value
  } else {
    input.document_id = null
  }
  if (needs.value !== 'resource') input.resource_id = null
  if (needs.value === 'resource' && !input.resource_id) return (error.value = '请选择网课')
  if (needs.value === 'section' && !input.section_id) return (error.value = '请选择章节')
  saving.value = true
  try {
    emit('saved', await PathService.saveStep(input))
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    saving.value = false
  }
}
</script>
