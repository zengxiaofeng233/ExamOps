<template>
  <div class="flex h-full flex-col">
    <header class="border-b border-slate-200 bg-surface px-6 py-4">
      <h1 class="page-title">设置</h1>
      <p class="mt-0.5 text-sm text-slate-500">所有设置和学习数据仅保存在本机。</p>
    </header>
    <div class="flex min-h-0 flex-1">
      <nav class="w-44 shrink-0 space-y-0.5 border-r border-slate-200 bg-surface p-2" aria-label="设置分类">
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
              <p class="hint mt-3">备份时关闭 ExamOps 后复制整个数据目录即可。可以将数据库、资料和预览缓存迁移到其他磁盘。</p>
              <div class="mt-4 flex flex-wrap gap-2">
                <button class="btn btn-secondary" @click="openDir">打开数据目录</button>
                <button class="btn btn-secondary" :disabled="storageBusy || !storage" @click="chooseDirectory">更改存储位置</button>
              </div>
              <div v-if="storage?.pending_dir" class="mt-4 rounded-lg border border-indigo-200 bg-indigo-50 p-3 text-sm text-slate-700" role="status">
                <p>下次启动时迁移到：</p>
                <p class="mt-1 break-all font-mono text-xs">{{ storage.pending_dir }}</p>
                <p class="mt-2">请等待导入、AI 生成等任务完成，关闭所有 ExamOps 窗口后重新打开。迁移期间启动可能较慢，请勿关闭程序。当前仍使用原目录。</p>
                <button class="btn btn-secondary mt-3" :disabled="storageBusy" @click="cancelMigration">取消迁移</button>
              </div>
              <p class="hint mt-3">迁移成功后旧数据暂时保留，可在此确认清理。系统目录仍会保留定位配置及运行时文件。</p>
              <div v-for="directory in storage?.previous_dirs ?? []" :key="directory" class="mt-4 rounded-lg border border-slate-200 p-3 text-sm text-slate-700">
                <p>迁移已完成，旧副本尚未清理：</p>
                <p class="mt-1 break-all font-mono text-xs">{{ directory }}</p>
                <button class="btn btn-secondary mt-3" :disabled="storageBusy" @click="cleanupDirectory = directory">清理旧数据</button>
              </div>
            </section>
          </template>

          <template v-else-if="active === 'about'">
            <section class="card p-5" aria-labelledby="update-heading">
              <div class="flex flex-wrap items-center justify-between gap-3">
                <h2 id="update-heading" class="section-title">软件更新</h2>
                <button type="button" class="btn btn-secondary" :disabled="updates.checking" @click="updates.check">
                  {{ updates.checking ? '正在检查…' : '检查更新' }}
                </button>
              </div>
              <p class="mt-2 text-sm text-slate-500">每次启动后台检查 GitHub 正式版本，仅提示更新，不会自动下载或安装。</p>
              <p v-if="updates.error" class="mt-3 text-sm text-rose-600" role="alert">{{ updates.error }}</p>
              <div v-else-if="updates.available" class="mt-3 text-sm text-slate-700">
                <p>发现新版本 v{{ updates.available.version }}（当前 v{{ updates.available.current_version }}）</p>
                <p class="mt-1 break-all text-xs text-slate-500">{{ updates.available.asset_name }}</p>
                <button type="button" class="btn btn-primary mt-3" @click="updates.openRelease">前往 GitHub 下载</button>
              </div>
              <p v-else-if="updates.checked && !updates.checking" class="mt-3 text-sm text-slate-600" role="status">未发现更新的 Windows x64 安装包。</p>
            </section>
            <section class="card p-5">
              <h2 class="section-title">关于 ExamOps</h2>
              <p class="mt-2 text-sm text-slate-600">基于个人学习资料的 AI 智能复习桌面软件。单用户、本地运行，无账号、无云端存储。</p>
              <dl v-if="info" class="mt-3 grid grid-cols-[8rem_minmax(0,1fr)] gap-y-2 text-sm">
                <dt class="text-slate-500">版本</dt>
                <dd class="text-slate-700">{{ info.version }}</dd>
                <dt class="text-slate-500">平台</dt>
                <dd class="text-slate-700">{{ info.platform }}</dd>
                <dt class="text-slate-500">许可证</dt>
                <dd class="text-slate-700">MIT</dd>
              </dl>
              <p class="mt-3 text-xs text-slate-500">
                学习方法设计参考了 Universal ExamPrep Skill（MIT 许可）。ExamOps 使用了许多开源组件，感谢它们的作者。
              </p>
              <button class="btn btn-secondary mt-3" @click="showNotices">查看开源许可</button>
            </section>
            <section class="card p-5">
              <h2 class="section-title">隐私</h2>
              <ul class="mt-2 list-disc space-y-1 pl-5 text-sm text-slate-600">
                <li>没有账号，没有云端存储；课程资料、学习记录和设置只保存在本机的数据目录。</li>
                <li>只有在你使用 AI 功能时，相关资料片段和问题才会发送到你自己配置的 AI 接口；ExamOps 不经过任何中转服务器。</li>
                <li>启动时和手动检查更新时会连接 GitHub 获取公开版本信息，不上传课程资料、学习记录或 API Key。</li>
                <li>API Key 优先保存在 Windows 凭据管理器中（不可用时保存在本机数据库并提示），界面只显示首尾几位。</li>
              </ul>
            </section>
          </template>
        </div>
      </div>
    </div>
    <AppModal :open="notices !== null" title="开源许可" size="xl" @close="notices = null">
      <MarkdownContent v-if="notices" :text="notices" class="text-sm" />
    </AppModal>
    <ConfirmDialog :open="selectedDirectory !== null" title="更改存储位置" confirm-text="确认，下次启动迁移"
      :busy="storageBusy" :message="`将数据库、资料副本、图片和缓存整体迁移到：\n${selectedDirectory ?? ''}\n\n请选择空文件夹。原始导入文件不移动，旧数据在验证成功后仍会保留，供你另行确认清理。\n\n确认后请等待当前任务完成，再关闭并重新打开 ExamOps。`"
      @cancel="!storageBusy && (selectedDirectory = null)" @confirm="scheduleMigration" />
    <ConfirmDialog :open="cleanupDirectory !== null" title="清理旧数据" confirm-text="确认清理" danger
      :busy="storageBusy" :message="`请先确认新目录中的资料、图片和学习记录正常。\n\n将删除以下旧目录中已迁移且未变化的数据库、资料副本和缓存：\n${cleanupDirectory ?? ''}\n\n当前数据、原始导入文件和其他文件不会删除。此操作无法撤销。`"
      @cancel="!storageBusy && (cleanupDirectory = null)" @confirm="cleanupPrevious" />
  </div>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import MarkdownContent from '@/components/chat/MarkdownContent.vue'
import ModelSettings from '@/components/settings/ModelSettings.vue'
import AppModal from '@/components/ui/AppModal.vue'
import ConfirmDialog from '@/components/ui/ConfirmDialog.vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useAppStore } from '@/stores/app'
import { useToastStore } from '@/stores/toast'
import { useUpdateStore } from '@/stores/updates'
import { AppService } from '@/services/app'
import type { AppInfo, AppSettings, StorageInfo } from '@/types/app'
import { QUESTION_KIND_HINTS, QUESTION_KIND_LABELS, TRIGGER_LABELS } from '@/utils/assessment'
import { formatBytes } from '@/utils/format'

const app = useAppStore()
const toast = useToastStore()
const updates = useUpdateStore()
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
const storage = ref<StorageInfo | null>(null)
const storageBusy = ref(false)
const selectedDirectory = ref<string | null>(null)
const cleanupDirectory = ref<string | null>(null)

async function chooseDirectory() {
  try {
    const directory = await open({ directory: true, multiple: false, title: '选择新的数据存储位置（空文件夹）' })
    if (typeof directory === 'string') selectedDirectory.value = directory
  } catch (e) { toast.error(e) }
}

async function scheduleMigration() {
  if (!selectedDirectory.value || storageBusy.value) return
  storageBusy.value = true
  try {
    await AppService.scheduleMigration(selectedDirectory.value)
    selectedDirectory.value = null
    storage.value = await AppService.storageInfo()
    toast.success('已安排迁移，请等待任务完成后关闭并重新打开 ExamOps')
  } catch (e) { toast.error(e) } finally { storageBusy.value = false }
}

async function cancelMigration() {
  storageBusy.value = true
  try {
    await AppService.scheduleMigration(null)
    storage.value = await AppService.storageInfo()
    toast.success('已取消迁移')
  } catch (e) { toast.error(e) } finally { storageBusy.value = false }
}

async function cleanupPrevious() {
  if (!cleanupDirectory.value || storageBusy.value) return
  storageBusy.value = true
  try {
    await AppService.cleanupPreviousData(cleanupDirectory.value)
    cleanupDirectory.value = null
    storage.value = await AppService.storageInfo()
    toast.success('旧数据已清理')
  } catch (e) { toast.error(e) } finally { storageBusy.value = false }
}
/** Plain copy (reactive proxies cannot be structured-cloned). */
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value))
const form = reactive<AppSettings>(clone(app.settings))

onMounted(async () => {
  if (!app.ready) await app.load().catch(() => undefined)
  Object.assign(form, clone(app.settings))
  info.value = await AppService.info().catch((e) => { toast.error(e); return null })
  storage.value = await AppService.storageInfo().catch((e) => { toast.error(e); return null })
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

const notices = ref<string | null>(null)
async function showNotices() {
  try {
    notices.value = await AppService.notices()
  } catch (e) {
    toast.error(e)
  }
}

async function openDir() {
  try { await AppService.openDataDir() } catch (e) { toast.error(e) }
}
</script>
