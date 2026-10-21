import { Channel, invoke } from '@tauri-apps/api/core'
import type { AppInfo, AppSettings, AvailableUpdate, LastLocation, StorageInfo } from '@/types/app'

export const AppService = {
  settings: () => invoke<AppSettings>('get_app_settings'),
  updateSettings: (settings: AppSettings) => invoke<AppSettings>('update_app_settings', { settings }),
  info: () => invoke<AppInfo>('get_app_info'),
  checkUpdates: () => invoke<AvailableUpdate | null>('check_for_updates'),
  downloadUpdate: (version: string, progress: (value: { downloaded: number; total: number | null }) => void) => {
    const onProgress = new Channel<{ downloaded: number; total: number | null }>()
    onProgress.onmessage = progress
    return invoke<void>('download_update', { version, onProgress })
  },
  installUpdate: () => invoke<void>('install_update'),
  openDataDir: () => invoke<void>('open_data_dir'),
  storageInfo: () => invoke<StorageInfo>('get_storage_info'),
  scheduleMigration: (directory: string | null) => invoke<void>('schedule_data_migration', { directory }),
  cleanupPreviousData: (directory: string) => invoke<void>('cleanup_previous_data', { directory }),
  openExternal: (url: string) => invoke<void>('open_external_url', { url }),
  saveLocation: (location: LastLocation) => invoke<void>('save_last_location', { location }),
  lastLocation: () => invoke<LastLocation | null>('get_last_location'),
  notices: () => invoke<string>('get_third_party_notices')
}
