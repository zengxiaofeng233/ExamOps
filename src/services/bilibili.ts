import { invoke } from '@tauri-apps/api/core'
import type { BilibiliSession, BilibiliQr, BilibiliLoginPoll, BilibiliText } from '@/types/bilibili'

export const BilibiliService = {
  session: () => invoke<BilibiliSession>('get_bilibili_session'),
  startLogin: () => invoke<BilibiliQr>('start_bilibili_login'),
  pollLogin: (key: string) => invoke<BilibiliLoginPoll>('poll_bilibili_login', { key }),
  cancelLogin: (key: string) => invoke<void>('cancel_bilibili_login', { key }),
  forgetLogin: () => invoke<void>('forget_bilibili_login'),
  fetchText: (url: string) => invoke<BilibiliText>('fetch_bilibili_text', { url })
}
