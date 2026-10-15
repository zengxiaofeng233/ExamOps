<template>
  <AppModal :open="open" :title="profile ? '编辑 AI 模型' : '添加 AI 模型'" size="lg" @close="emit('close')">
    <!-- Backend validation gives clearer messages than the browser (fields may sit in a closed <details>). -->
    <form id="profile-form" class="space-y-4" novalidate @submit.prevent="save">
      <div v-if="!profile">
        <label class="label" for="preset">服务商预设</label>
        <select id="preset" class="input" @change="applyPreset(($event.target as HTMLSelectElement).value)">
          <option value="">选择预设以填入接口地址（可修改）</option>
          <option v-for="preset in PRESETS" :key="preset.label" :value="preset.label">{{ preset.label }}</option>
        </select>
        <p class="hint">支持任何兼容 OpenAI Chat Completions 的接口，包括本机运行的 Ollama、LM Studio。</p>
      </div>
      <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <div>
          <label class="label" for="p-name">配置名称</label>
          <input id="p-name" v-model="form.name" class="input" required maxlength="40" placeholder="例如：DeepSeek" />
        </div>
        <div>
          <label class="label" for="p-model">模型名称</label>
          <input id="p-model" v-model="form.model" class="input" required placeholder="例如：deepseek-chat" />
        </div>
      </div>
      <div>
        <label class="label" for="p-url">接口地址（Base URL）</label>
        <div class="flex gap-2">
          <input id="p-url" v-model="form.base_url" class="input min-w-0 flex-1 font-mono" required placeholder="https://api.example.com/v1" />
          <button type="button" class="btn btn-secondary shrink-0" :disabled="fetchingModels || !form.base_url.trim()" @click="fetchModels">
            {{ fetchingModels ? '获取中…' : '获取模型' }}
          </button>
        </div>
        <p class="hint">填写到 <code>/v1</code> 一级即可，系统会自动补全 <code>/chat/completions</code>。</p>
      </div>
      <div>
        <label class="label" for="p-key">API Key</label>
        <input
          id="p-key"
          v-model="apiKey"
          type="password"
          autocomplete="off"
          class="input font-mono"
          :placeholder="profile?.has_key ? `已保存（${profile.key_hint ?? '••••'}），留空则保持不变` : '本地模型可留空'"
        />
        <p class="hint">
          密钥保存在系统凭据管理器中，不会写入课程数据，也不会显示在界面上。
          <button v-if="profile?.has_key" type="button" class="text-rose-600 hover:underline" @click="removeKey = true; apiKey = ''">删除已保存的密钥</button>
          <span v-if="removeKey" class="text-rose-600">（保存后删除）</span>
        </p>
      </div>
      <div v-if="availableModels.length">
        <label class="label" for="p-available-models">可用模型（{{ availableModels.length }}）</label>
        <select id="p-available-models" class="input" :value="availableModels.includes(form.model) ? form.model : ''" @change="form.model = ($event.target as HTMLSelectElement).value">
          <option disabled value="">选择模型以填入模型名称</option>
          <option v-for="model in availableModels" :key="model" :value="model">{{ model }}</option>
        </select>
        <p class="hint">列表由服务商返回；是否支持对话及调用权限请使用“测试连接”确认。也可手动填写模型名称。</p>
      </div>
      <p v-if="modelListMessage" class="text-sm text-slate-600" role="status">{{ modelListMessage }}</p>
      <p v-if="modelListError" class="text-sm text-rose-600" role="alert">{{ modelListError }} 可继续手动填写模型名称。</p>
      <details class="rounded-lg border border-slate-200 px-4 py-3" :open="advancedOpen">
        <summary class="cursor-pointer text-sm font-medium text-slate-700">模型参数与 Token 预算</summary>
        <div class="mt-3 grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label class="label" for="p-ctx">上下文预算（tokens）</label>
            <input id="p-ctx" v-model.number="form.context_window" type="number" min="2048" step="1" class="input" />
            <p class="hint">单次请求最多发送的 token 数；可小于模型真实上限以节省费用。</p>
          </div>
          <div>
            <label class="label" for="p-out">最大输出（tokens）</label>
            <input id="p-out" v-model.number="form.max_output_tokens" type="number" min="64" step="1" class="input" />
          </div>
          <div>
            <label class="label" for="p-temp">温度 {{ form.temperature.toFixed(1) }}</label>
            <input id="p-temp" v-model.number="form.temperature" type="range" min="0" max="2" step="0.1" class="w-full accent-indigo-600" />
            <p class="hint">越低越稳定，讲解与判分建议 0.2–0.5。</p>
          </div>
          <div>
            <label class="label" for="p-timeout">超时（秒）</label>
            <input id="p-timeout" v-model.number="form.timeout_secs" type="number" min="10" max="900" class="input" />
          </div>
          <label class="flex items-start gap-2 sm:col-span-2">
            <input v-model="form.json_mode" type="checkbox" class="mt-1 accent-indigo-600" />
            <span class="text-sm text-slate-700">接口支持 JSON 模式（response_format）<span class="block text-xs text-slate-500">生成小测、提取知识点时要求模型只输出 JSON。不确定时保持关闭。</span></span>
          </label>
        </div>
      </details>
      <div v-if="testResult" class="rounded-lg border px-4 py-3 text-sm" :class="testResult.ok ? 'border-emerald-200 bg-emerald-50 text-emerald-800' : 'border-rose-200 bg-rose-50 text-rose-800'" role="status">
        <template v-if="testResult.ok">连接成功：{{ testResult.latency }} ms，模型回复“{{ testResult.reply }}”</template>
        <template v-else>{{ testResult.error }}</template>
      </div>
      <p v-if="error" class="text-sm text-rose-600" role="alert">{{ error }}</p>
    </form>
    <template #footer>
      <button type="button" class="btn btn-secondary mr-auto" :disabled="testing || saving" @click="test">
        {{ testing ? '测试中…' : '测试连接' }}
      </button>
      <button type="button" class="btn btn-secondary" @click="emit('close')">取消</button>
      <button type="submit" form="profile-form" class="btn btn-primary" :disabled="saving">{{ saving ? '保存中…' : '保存' }}</button>
    </template>
  </AppModal>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import AppModal from '@/components/ui/AppModal.vue'
import { ModelService } from '@/services/ai'
import type { ModelProfile, ProfileInput, SaveProfileResult } from '@/types/ai'
import { errorMessage } from '@/utils/errors'

const PRESETS = [
  { label: 'DeepSeek', base_url: 'https://api.deepseek.com/v1', model: 'deepseek-chat', context: 64000 },
  { label: 'OpenAI', base_url: 'https://api.openai.com/v1', model: 'gpt-4o-mini', context: 128000 },
  { label: '通义千问（阿里云百炼）', base_url: 'https://dashscope.aliyuncs.com/compatible-mode/v1', model: 'qwen-plus', context: 128000 },
  { label: '智谱 GLM', base_url: 'https://open.bigmodel.cn/api/paas/v4', model: 'glm-4-flash', context: 128000 },
  { label: 'Kimi（月之暗面）', base_url: 'https://api.moonshot.cn/v1', model: 'moonshot-v1-32k', context: 32000 },
  { label: '硅基流动', base_url: 'https://api.siliconflow.cn/v1', model: 'Qwen/Qwen2.5-7B-Instruct', context: 32000 },
  { label: 'Ollama（本机）', base_url: 'http://localhost:11434/v1', model: 'qwen2.5:7b', context: 8192 },
  { label: 'LM Studio（本机）', base_url: 'http://localhost:1234/v1', model: 'local-model', context: 8192 }
]

const props = defineProps<{ open: boolean; profile: ModelProfile | null }>()
const emit = defineEmits<{ close: []; saved: [result: SaveProfileResult] }>()

const blank = (): Omit<ProfileInput, 'api_key'> => ({
  id: null, name: '', base_url: '', model: '', temperature: 0.3, top_p: null,
  max_output_tokens: 2048, context_window: 32000, timeout_secs: 120, json_mode: false
})
const form = reactive(blank())
const apiKey = ref('')
const removeKey = ref(false)
const saving = ref(false)
const testing = ref(false)
const fetchingModels = ref(false)
const availableModels = ref<string[]>([])
const modelListMessage = ref('')
const modelListError = ref('')
let modelListRequest = 0

// Discard results belonging to an old address, key, or dialog session.
watch(() => [props.open, props.profile?.id, form.base_url, apiKey.value, removeKey.value], () => {
  modelListRequest++
  fetchingModels.value = false
  availableModels.value = []
  modelListMessage.value = ''
  modelListError.value = ''
}, { flush: 'sync' })
const error = ref('')
const advancedOpen = ref(false)
const testResult = ref<{ ok: boolean; latency?: number; reply?: string; error?: string } | null>(null)

watch(() => props.open, (open) => {
  if (!open) return
  Object.assign(form, blank(), props.profile ? {
    id: props.profile.id, name: props.profile.name, base_url: props.profile.base_url, model: props.profile.model,
    temperature: props.profile.temperature, top_p: props.profile.top_p, max_output_tokens: props.profile.max_output_tokens,
    context_window: props.profile.context_window, timeout_secs: props.profile.timeout_secs, json_mode: props.profile.json_mode
  } : {})
  apiKey.value = ''
  removeKey.value = false
  error.value = ''
  testResult.value = null
  advancedOpen.value = false
})

function applyPreset(label: string) {
  const preset = PRESETS.find(p => p.label === label)
  if (!preset) return
  form.name = form.name || preset.label
  form.base_url = preset.base_url
  form.model = preset.model
  form.context_window = preset.context
  if (preset.context <= 8192) form.max_output_tokens = 1024
}

function payload(): ProfileInput {
  const key = apiKey.value.trim()
  return { ...form, api_key: key ? key : removeKey.value ? '' : props.profile ? null : '' }
}

async function fetchModels() {
  const request = ++modelListRequest
  fetchingModels.value = true
  availableModels.value = []
  modelListMessage.value = ''
  modelListError.value = ''
  const input = payload()
  try {
    const models = await ModelService.fetchModels({
      id: input.id, base_url: input.base_url, api_key: input.api_key, timeout_secs: input.timeout_secs
    })
    if (request !== modelListRequest) return
    availableModels.value = models
    if (!models.length) modelListMessage.value = '服务商返回的模型列表为空，可手动填写模型名称。'
  } catch (e) {
    if (request === modelListRequest) modelListError.value = errorMessage(e)
  } finally {
    if (request === modelListRequest) fetchingModels.value = false
  }
}

async function test() {
  testing.value = true
  testResult.value = null
  try {
    const report = await ModelService.test(payload())
    testResult.value = { ok: true, latency: report.latency_ms, reply: report.reply }
  } catch (e) {
    testResult.value = { ok: false, error: errorMessage(e) }
  } finally {
    testing.value = false
  }
}

async function save() {
  saving.value = true
  error.value = ''
  try {
    emit('saved', await ModelService.save(payload()))
  } catch (e) {
    error.value = errorMessage(e)
    advancedOpen.value = /tokens|温度|超时|Top P/.test(error.value)
  } finally {
    saving.value = false
  }
}
</script>
