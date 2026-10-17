export type Theme = 'system' | 'light' | 'dark'
export interface AppearanceSettings {
  theme: Theme
  fit: 'cover' | 'contain' | 'stretch' | 'center'
  positionX: number
  positionY: number
  imageOpacity: number
  panelOpacity: number
  overlayOpacity: number
  blur: number
  brightness: number
  overlayColor: string
  reducedMotion: boolean
}
export const defaults = (): AppearanceSettings => ({
  theme: 'system',
  fit: 'cover',
  positionX: 50,
  positionY: 50,
  imageOpacity: 65,
  panelOpacity: 94,
  overlayOpacity: 20,
  blur: 6,
  brightness: 100,
  overlayColor: '#172039',
  reducedMotion: false,
})
export interface Appearance {
  settings: AppearanceSettings
  avatar: string | null
  background: string | null
  warnings: string[]
}
export interface Bootstrap {
  appearance: Appearance
  hasSession: boolean
  canClearSession: boolean
  authorizationReady: boolean
  warning: string | null
}
export interface CloudFile {
  id: string
  name: string
  kind: string
  size: number
  modified_time: string
  mime_type: string
}
export interface Transfer {
  id: string
  file_id: string
  file_name: string
  completed: number
  total: number | null
  phase: 'Queued' | 'Running' | 'Interrupted' | 'Completed' | 'Cancelled' | 'Failed'
}
export interface Transfers {
  records: Transfer[]
  errors: Record<string, string>
  speeds: Record<string, number>
  persistence_error: string | null
}
export interface AppError {
  code: string
  message: string
}
export const isFolder = (file: CloudFile) => file.kind === 'drive#folder'
export function formatBytes(size: number) {
  if (!size) return '0 字节'
  const units = ['字节', 'KB', 'MB', 'GB', 'TB']
  const power = Math.min(Math.floor(Math.log(size) / Math.log(1024)), 4)
  return `${(size / 1024 ** power).toLocaleString('zh-CN', { maximumFractionDigits: 1 })} ${units[power]}`
}

export function formatDate(value: string) {
  if (!value) return '—'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN')
}
