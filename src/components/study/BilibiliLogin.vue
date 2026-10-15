<template>
  <div class="space-y-1">
    <div class="flex flex-wrap items-center gap-2 text-xs text-slate-500">
      <span>{{ session.logged_in ? `B 站已登录${session.name ? `：${session.name}` : ''}` : '部分字幕和 AI 总结需要登录 B 站' }}</span>
      <button type="button" class="btn btn-secondary btn-sm" @click="start">{{ session.logged_in ? '重新登录' : '扫码登录 B 站' }}</button>
      <button v-if="session.logged_in" type="button" class="btn btn-ghost btn-sm" :disabled="forgetting" @click="forget">清除登录</button>
    </div>
    <p v-if="sessionError" class="text-xs text-amber-700" role="status">{{ sessionError }}</p>
    <AppModal :open="open" title="扫码登录 B 站" size="sm" @close="close">
      <div class="space-y-3 text-center">
        <p class="text-sm text-slate-600">使用哔哩哔哩手机客户端扫码，并在手机上确认登录。</p>
        <img v-if="qr" :src="qr.image" alt="B 站登录二维码" class="mx-auto size-60 rounded bg-white p-2" />
        <p class="text-sm text-slate-600" role="status">{{ status }}</p>
        <button v-if="retry" type="button" class="btn btn-secondary" @click="start">重新获取二维码</button>
        <p class="text-xs leading-5 text-slate-500">登录凭据仅保存在本机系统凭据管理器，用于读取 B 站网课文字，不会发给 AI。可随时清除本应用的登录信息。</p>
      </div>
    </AppModal>
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import AppModal from '@/components/ui/AppModal.vue'
import { BilibiliService } from '@/services/bilibili'
import type { BilibiliQr, BilibiliSession } from '@/types/bilibili'
import { errorMessage } from '@/utils/errors'

const emit = defineEmits<{ loggedIn: [] }>()
const session = ref<BilibiliSession>({ logged_in: false, name: '' })
const sessionError = ref('')
const forgetting = ref(false)
const open = ref(false)
const qr = ref<BilibiliQr | null>(null)
const status = ref('')
const retry = ref(false)
let timer: ReturnType<typeof setTimeout> | undefined
let generation = 0
let active = true

onMounted(async () => {
  const version = generation
  try {
    const result = await BilibiliService.session()
    if (active && version === generation) session.value = result
  } catch (error) { if (active && version === generation) sessionError.value = errorMessage(error) }
})

function close() {
  generation++
  open.value = false
  clearTimeout(timer)
  if (qr.value) void BilibiliService.cancelLogin(qr.value.key).catch(() => undefined)
  qr.value = null
}

onBeforeUnmount(() => { active = false; close() })

async function start() {
  close()
  const version = generation
  open.value = true
  retry.value = false
  status.value = '正在获取二维码…'
  try {
    const result = await BilibiliService.startLogin()
    if (!active || version !== generation) {
      await BilibiliService.cancelLogin(result.key)
      return
    }
    qr.value = result
    status.value = '等待扫码'
    timer = setTimeout(() => poll(version, result.key), 2000)
  } catch (error) {
    if (version !== generation || !active) return
    status.value = errorMessage(error)
    retry.value = true
  }
}

async function poll(version: number, key: string) {
  try {
    const result = await BilibiliService.pollLogin(key)
    if (version !== generation || !active) return
    if (result.status === 'success') {
      session.value = { logged_in: true, name: '' }
      sessionError.value = ''
      close()
      emit('loggedIn')
      return
    }
    if (result.status === 'expired') {
      status.value = '二维码已过期，请重新获取'
      retry.value = true
      return
    }
    status.value = result.status === 'scanned' ? '已扫码，请在手机上确认登录' : '等待扫码'
    timer = setTimeout(() => poll(version, key), 2000)
  } catch (error) {
    if (version !== generation || !active) return
    status.value = errorMessage(error)
    retry.value = true
  }
}

async function forget() {
  forgetting.value = true
  close()
  try {
    await BilibiliService.forgetLogin()
    session.value = { logged_in: false, name: '' }
    sessionError.value = ''
  } catch (error) { sessionError.value = errorMessage(error) }
  finally { forgetting.value = false }
}
</script>
