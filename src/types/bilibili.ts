export interface BilibiliSession { logged_in: boolean; name: string }
export interface BilibiliQr { key: string; image: string }
export interface BilibiliLoginPoll { status: 'waiting' | 'scanned' | 'expired' | 'success' }
export interface BilibiliText {
  title: string
  url: string
  kind: 'subtitle' | 'summary'
  transcript: string
  notice: string
}
