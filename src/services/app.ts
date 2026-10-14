import { invoke } from '@tauri-apps/api/core'
import type { AppInfo, AppSettings, LastLocation } from '@/types/app'

export const AppService = {
  settings: () => invoke<AppSettings>('get_app_settings'),
  updateSettings: (settings: AppSettings) => invoke<AppSettings>('update_app_settings', { settings }),
  info: () => invoke<AppInfo>('get_app_info'),
  openDataDir: () => invoke<void>('open_data_dir'),
  openExternal: (url: string) => invoke<void>('open_external_url', { url }),
  saveLocation: (location: LastLocation) => invoke<void>('save_last_location', { location }),
  lastLocation: () => invoke<LastLocation | null>('get_last_location')
}
