<template>
  <div class="space-y-5">
    <section class="card p-5">
      <div class="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 class="section-title">AI 模型</h2>
          <p class="mt-1 text-sm text-slate-500">使用你自己的 API（BYOK）。只有在你提问、生成小测等主动操作时才会联网调用。</p>
        </div>
        <button class="btn btn-primary" @click="edit(null)"><PlusIcon class="size-4" />添加模型</button>
      </div>
      <p v-if="loading" class="mt-4 text-sm text-slate-400">加载中…</p>
      <EmptyState v-else-if="!models.profiles.length" title="还没有配置 AI 模型" description="添加一个兼容 OpenAI 的接口后，即可基于教材问答、AI 讲解和生成小测。" />
      <ul v-else class="mt-4 divide-y divide-slate-100 rounded-lg border border-slate-200">
        <li v-for="profile in models.profiles" :key="profile.id" class="flex flex-wrap items-center gap-3 px-4 py-3">
          <div class="min-w-0 flex-1">
            <p class="flex items-center gap-2 text-sm font-medium text-slate-900">
              {{ profile.name }}
              <span v-if="profile.is_default" class="badge bg-indigo-50 text-indigo-700">默认</span>
              <span v-if="profile.key_storage === 'local'" class="badge bg-amber-50 text-amber-700" title="系统凭据管理器不可用，密钥保存在本机数据库">密钥本地保存</span>
            </p>
            <p class="mt-0.5 truncate text-xs text-slate-500">{{ profile.model }} · {{ profile.base_url }}</p>
            <p class="text-xs text-slate-400">上下文预算 {{ profile.context_window.toLocaleString() }} · 最大输出 {{ profile.max_output_tokens.toLocaleString() }} · {{ profile.has_key ? `密钥 ${profile.key_hint ?? ''}` : '未设置密钥' }}</p>
          </div>
          <div class="flex gap-1">
            <button class="btn btn-ghost btn-sm" :disabled="busy === profile.id" @click="test(profile)">{{ busy === profile.id ? '测试中…' : '测试' }}</button>
            <button v-if="!profile.is_default" class="btn btn-ghost btn-sm" @click="makeDefault(profile)">设为默认</button>
            <button class="btn btn-ghost btn-sm" @click="edit(profile)">编辑</button>
            <button class="btn btn-ghost btn-sm text-rose-600 hover:text-rose-700" @click="removing = profile">删除</button>
          </div>
        </li>
      </ul>
    </section>

    <section class="card p-5">
      <h2 class="section-title">上下文策略</h2>
      <p class="mt-1 text-sm text-slate-500">决定 AI 回答时能看到多少资料。新对话默认使用这里的设置，也可以在对话中单独切换。</p>
      <div class="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-3">
        <label v-for="option in CONTEXT_OPTIONS" :key="option.value"
          class="flex cursor-pointer flex-col rounded-lg border p-3 text-sm"
          :class="form.mode === option.value ? 'border-indigo-500 bg-indigo-50/50 ring-1 ring-indigo-500' : 'border-slate-200 hover:border-slate-300'">
          <span class="flex items-center gap-2 font-medium text-slate-800">
            <input v-model="form.mode" type="radio" :value="option.value" class="accent-indigo-600" @change="saveContext" />{{ option.label }}
          </span>
          <span class="mt-1 text-xs text-slate-500">{{ option.description }}</span>
        </label>
      </div>
      <div class="mt-4 grid grid-cols-1 gap-4 sm:grid-cols-3">
        <div>
          <label class="label" for="c-topk">检索片段数</label>
          <input id="c-topk" v-model.number="form.rag_top_k" type="number" min="2" max="30" class="input" @change="saveContext" />
        </div>
        <div>
          <label class="label" for="c-ratio">全文模式阈值 {{ Math.round(form.full_ratio * 100) }}%</label>
          <input id="c-ratio" v-model.number="form.full_ratio" type="range" min="0.2" max="1" step="0.05" class="w-full accent-indigo-600" @change="saveContext" />
          <p class="hint">资料占上下文预算不超过该比例时，自动模式发送全文。</p>
        </div>
        <div>
          <label class="label" for="c-history">携带历史轮数</label>
          <input id="c-history" v-model.number="form.history_turns" type="number" min="0" max="20" class="input" @change="saveContext" />
        </div>
      </div>
    </section>

    <section v-if="usage" class="card p-5">
      <h2 class="section-title">Token 用量</h2>
      <dl class="mt-3 grid grid-cols-2 gap-3 text-sm sm:grid-cols-4">
        <div><dt class="text-xs text-slate-500">今日输入</dt><dd class="text-lg font-semibold">{{ usage.today_prompt.toLocaleString() }}</dd></div>
        <div><dt class="text-xs text-slate-500">今日输出</dt><dd class="text-lg font-semibold">{{ usage.today_completion.toLocaleString() }}</dd></div>
        <div><dt class="text-xs text-slate-500">累计 tokens</dt><dd class="text-lg font-semibold">{{ (usage.total_prompt + usage.total_completion).toLocaleString() }}</dd></div>
        <div><dt class="text-xs text-slate-500">请求次数</dt><dd class="text-lg font-semibold">{{ usage.requests }}</dd></div>
      </dl>
      <p class="hint mt-2">接口未返回用量时按字数估算，仅供参考；实际费用以服务商账单为准。</p>
    </section>

    <ProfileDialog :open="dialogOpen" :profile="editing" @close="dialogOpen = false" @saved="onSaved" />
    <ConfirmDialog
      :open="!!removing"
      title="删除 AI 模型"
      :message="`确定删除“${removing?.name}”吗？保存在系统凭据管理器中的密钥也会被删除。`"
      confirm-text="删除"
      danger
      @cancel="removing = null"
      @confirm="remove"
    />
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { PlusIcon } from '@heroicons/vue/20/solid'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import ProfileDialog from './ProfileDialog.vue'
import { ModelService } from '@/services/ai'
import { useAppStore } from '@/stores/app'
import { useModelStore } from '@/stores/models'
import { useToastStore } from '@/stores/toast'
import type { ModelProfile, SaveProfileResult, UsageSummary } from '@/types/ai'
import type { ContextSettings } from '@/types/app'

const CONTEXT_OPTIONS = [
  { value: 'auto', label: '自动（推荐）', description: '资料能放进上下文时发送全文，否则检索相关片段。' },
  { value: 'full', label: '全文', description: '总是发送所选范围的全部资料，超出预算时截断。' },
  { value: 'rag', label: '检索', description: '只发送与问题最相关的片段，节省 token。' }
] as const

const app = useAppStore()
const models = useModelStore()
const toast = useToastStore()
const loading = ref(true)
const dialogOpen = ref(false)
const editing = ref<ModelProfile | null>(null)
const removing = ref<ModelProfile | null>(null)
const busy = ref<number | null>(null)
const usage = ref<UsageSummary | null>(null)
const form = reactive<ContextSettings>({ ...app.settings.context })

onMounted(async () => {
  try {
    await Promise.all([models.load(true), app.ready ? Promise.resolve() : app.load()])
    Object.assign(form, app.settings.context)
    usage.value = await ModelService.usage()
  } catch (e) {
    toast.error(e)
  } finally {
    loading.value = false
  }
})

function edit(profile: ModelProfile | null) {
  editing.value = profile
  dialogOpen.value = true
}

async function onSaved(result: SaveProfileResult) {
  dialogOpen.value = false
  await models.load(true)
  if (result.warning) toast.error(result.warning)
  else toast.success('模型配置已保存')
}

async function test(profile: ModelProfile) {
  busy.value = profile.id
  try {
    const report = await ModelService.test({ ...profile, id: profile.id, api_key: null })
    toast.success(`${profile.name} 连接成功（${report.latency_ms} ms）`)
    usage.value = await ModelService.usage()
  } catch (e) {
    toast.error(e)
  } finally {
    busy.value = null
  }
}

async function makeDefault(profile: ModelProfile) {
  try {
    await ModelService.setDefault(profile.id)
    await models.load(true)
  } catch (e) {
    toast.error(e)
  }
}

async function remove() {
  if (!removing.value) return
  try {
    await ModelService.remove(removing.value.id)
    removing.value = null
    await models.load(true)
  } catch (e) {
    toast.error(e)
  }
}

async function saveContext() {
  try {
    const saved = await app.save({ ...app.settings, context: { ...form } })
    Object.assign(form, saved.context)
  } catch (e) {
    toast.error(e)
  }
}
</script>
