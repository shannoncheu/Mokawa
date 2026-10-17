<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import {
  Home,
  Folder,
  Clock3,
  Star,
  Download,
  Share2,
  Trash2,
  Settings2,
  Search,
  Plus,
  Upload,
  List,
  Grid2X2,
  RefreshCw,
  ChevronRight,
  ChevronDown,
  X,
  Minus,
  Square,
  Copy,
  UserRound,
  ArrowLeft,
  Ellipsis,
  CircleAlert,
  Check,
  LoaderCircle,
  LogOut,
  Monitor,
  HardDrive,
  FileText,
  PanelLeftClose,
  PanelLeftOpen,
  Link2,
} from '@lucide/vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { call, desktop, errorMessage } from './bridge'
import { useAppearance } from './useAppearance'
import { useFileView } from './useFileView'
import {
  type Bootstrap,
  type CloudFile,
  type Transfers,
  type AppError,
  isFolder,
  formatBytes,
  formatDate,
  type DriveQuota,
  type OfflineTask,
  type Share,
} from './types'
import AppearancePanel from './components/AppearancePanel.vue'
import VirtualFiles from './components/VirtualFiles.vue'
const notice = ref('')
let noticeTimer: ReturnType<typeof setTimeout> | undefined
function notify(message: string) {
  notice.value = message
  if (noticeTimer) clearTimeout(noticeTimer)
  noticeTimer = setTimeout(() => {
    notice.value = ''
  }, 7000)
}
const appearance = useAppearance(notify)
const { settings, avatar, background, dark, style, saveStatus, importing } = appearance
const hasSavedSession = ref(false)
const modal = ref<HTMLFormElement>()
let returnFocus: HTMLElement | null = null
const page = ref('login')
const authenticated = ref(false)
const authMethod = ref<'oauth' | 'personal_access_token' | null>(null)
const busy = ref(false)
const personalAccessToken = ref('')
const booting = ref(true)
const status = ref('')
const files = shallowRef<CloudFile[]>([])
const selected = shallowRef(new Set<string>())
const quota = ref<DriveQuota | null>(null)
const quotaLoading = ref(false)
const quotaError = ref(false)
const quotaUpdatedAt = ref(0)
const path = ref<{ id: string; name: string }[]>([])
const query = ref('')
const debouncedQuery = ref('')
const descending = ref(false)
const grid = ref(false)
const detail = ref(false)
const accountMenu = ref(false)
const menu = ref<{ x: number; y: number } | null>(null)
const tasks = shallowRef<Transfers>({
  records: [],
  errors: {},
  speeds: {},
  persistence_error: null,
})
const offlineTasks = shallowRef<OfflineTask[]>([])
const offlineLoading = ref(false)
const offlineError = ref('')
const offlineSource = ref('')
const offlineDestination = ref('')
const taskKind = ref<'all' | 'local' | 'cloud'>('all')
const dialog = ref<'folder' | 'rename' | 'trash' | 'offline' | 'share' | null>(null)
const newName = ref('')
const writing = ref(false)
const nameInput = ref<HTMLInputElement>()
let requestGeneration = Date.now()
let remotePageGeneration = 0
let disposed = false
let queryTimer: ReturnType<typeof setTimeout> | undefined
let offlineTimer: ReturnType<typeof setInterval> | undefined
let resizedObserver: UnlistenFn | undefined
let taskObserver: UnlistenFn | undefined
let closeObserver: UnlistenFn | undefined
let closeFlush: Promise<void> | undefined
let titlebarPointer: { id: number; x: number; y: number } | undefined
const maximized = ref(false)
const sidebarCollapsed = ref(false)
const settingsSection = ref('appearance')
const taskFilter = ref<'all' | 'active' | 'completed' | 'failed'>('all')
const recentVisits = ref<{ file: CloudFile; visitedAt: string }[]>([])
const remoteFiles = shallowRef<CloudFile[]>([])
const shares = shallowRef<Share[]>([])
const pageLoading = ref(false)
const remoteError = ref('')
const shareExpiration = ref(7)
const sharePassword = ref(true)
watch([page, taskKind], ([currentPage, kind]) => {
  if (offlineTimer) clearInterval(offlineTimer)
  offlineTimer = undefined
  if (currentPage === 'offline' || (currentPage === 'tasks' && kind !== 'local')) {
    offlineTimer = setInterval(() => {
      const active = offlineTasks.value.some((task) =>
        ['PHASE_TYPE_RUNNING', 'PHASE_TYPE_PENDING'].includes(task.phase.toUpperCase()),
      )
      if (active) void refreshOffline()
    }, 10_000)
  }
})
const navigation = [
  { id: 'home', label: '首页', icon: Home },
  { id: 'files', label: '我的文件', icon: Folder },
  { id: 'recent', label: '最近访问', icon: Clock3 },
  { id: 'favorites', label: '收藏', icon: Star },
  { id: 'offline', label: '离线下载', icon: Download },
  { id: 'shares', label: '分享管理', icon: Share2 },
  { id: 'trash', label: '回收站', icon: Trash2 },
]
const settingsNavigation = [
  { id: 'appearance', label: '外观与个性化', icon: Monitor },
  { id: 'downloads', label: '下载', icon: Download },
  { id: 'account', label: '账号', icon: UserRound },
  { id: 'about', label: '关于 Mokawa', icon: CircleAlert },
]
const filteredTasks = computed(() => {
  const rows = [...tasks.value.records].reverse()
  if (taskFilter.value === 'active')
    return rows.filter((task) => ['Queued', 'Running'].includes(task.phase))
  if (taskFilter.value === 'completed') return rows.filter((task) => task.phase === 'Completed')
  if (taskFilter.value === 'failed')
    return rows.filter((task) => ['Failed', 'Interrupted'].includes(task.phase))
  return rows
})
const filteredCloudTasks = computed(() => {
  const rows = offlineTasks.value
  if (taskFilter.value === 'active')
    return rows.filter((task) =>
      ['PHASE_TYPE_RUNNING', 'PHASE_TYPE_PENDING'].includes(task.phase.toUpperCase()),
    )
  if (taskFilter.value === 'completed')
    return rows.filter((task) =>
      ['PHASE_TYPE_COMPLETE', 'COMPLETE', 'COMPLETED'].includes(task.phase.toUpperCase()),
    )
  if (taskFilter.value === 'failed')
    return rows.filter((task) =>
      ['PHASE_TYPE_ERROR', 'ERROR', 'FAILED'].includes(task.phase.toUpperCase()),
    )
  return rows
})
const recentTasks = computed(() => [...tasks.value.records].reverse().slice(0, 3))
const commonFolders = computed(() => files.value.filter(isFolder).slice(0, 6))
const { visible, indexing } = useFileView(files, debouncedQuery, descending, notify)
const currentFile = computed(() =>
  selected.value.size === 1 ? files.value.find((file) => selected.value.has(file.id)) : undefined,
)
const previewKind = computed(() => {
  const file = currentFile.value
  if (!file || isFolder(file)) return 'other'
  const mime = file.mime_type.toLowerCase()
  const ext = file.name.split('.').at(-1)?.toLowerCase() || ''
  if (
    mime.startsWith('image/') ||
    ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'avif'].includes(ext)
  )
    return 'image'
  if (mime.startsWith('video/') || ['mp4', 'webm', 'mov', 'm4v'].includes(ext)) return 'video'
  if (mime.startsWith('audio/') || ['mp3', 'm4a', 'aac', 'ogg', 'wav', 'flac'].includes(ext))
    return 'audio'
  if (
    mime.startsWith('text/') ||
    ['txt', 'md', 'json', 'csv', 'log', 'yaml', 'yml', 'xml'].includes(ext)
  )
    return 'text'
  return 'other'
})
const previewUrl = ref('')
const textContent = ref('')
const previewError = ref('')
const previewLoading = ref(false)
let previewGeneration = 0
let mediaElement: HTMLMediaElement | undefined
watch(
  [detail, page, () => currentFile.value?.id, previewKind],
  async ([open, currentPage, id, kind]) => {
    const generation = ++previewGeneration
    previewLoading.value = false
    previewUrl.value = ''
    textContent.value = ''
    previewError.value = ''
    if (mediaElement) {
      mediaElement.pause()
      mediaElement.removeAttribute('src')
      mediaElement.load()
      mediaElement = undefined
    }
    await call('cancel_preview', { generation }).catch(() => {})
    if (generation !== previewGeneration) return
    if (!open || currentPage !== 'files' || !id || kind === 'other') return
    previewLoading.value = true
    try {
      if (kind === 'text')
        textContent.value = await call<string>('text_preview', { id, generation })
      else previewUrl.value = await call<string>('preview_content_link', { id, generation })
    } catch (error) {
      if (generation === previewGeneration) {
        if (['unauthorized', 'no_session', 'invalid_session'].includes((error as AppError)?.code))
          report(error)
        else previewError.value = errorMessage(error)
      }
    } finally {
      if (generation === previewGeneration) previewLoading.value = false
    }
  },
)
const phases = {
  Queued: '排队中',
  Running: '下载中',
  Interrupted: '已中断，请重新下载',
  Completed: '已完成',
  Cancelled: '已取消',
  Failed: '失败',
}
watch(query, (value) => {
  if (queryTimer) clearTimeout(queryTimer)
  if (offlineTimer) clearInterval(offlineTimer)
  queryTimer = setTimeout(() => {
    debouncedQuery.value = value
  }, 160)
})
watch(page, (_value, old) => {
  if (old === 'files') {
    requestGeneration++
    void call('cancel_page').catch(() => {})
    busy.value = false
  }
  menu.value = null
  accountMenu.value = false
})
function route(id: string) {
  if (['settings', 'tasks', 'login', 'recent'].includes(id)) {
    page.value = id
    return
  }
  if (!authenticated.value) {
    page.value = 'login'
    notify('请先连接 PikPak 账号。')
    return
  }
  page.value = id
  if (id === 'files' && !files.value.length) void load()
  if (id === 'offline') void refreshOffline()
  if (id === 'favorites') void loadFavorites()
  if (id === 'shares') void loadShares()
  if (id === 'trash') void loadTrash()
}
function recordVisit(file: CloudFile) {
  try {
    const next = [
      { file, visitedAt: new Date().toISOString() },
      ...recentVisits.value.filter((row) => row.file.id !== file.id),
    ].slice(0, 100)
    recentVisits.value = next
    localStorage.setItem('mokawa-recent-visits', JSON.stringify(next))
  } catch {
    notify('无法保存本机最近访问记录。')
  }
}
function clearRecent() {
  recentVisits.value = []
  try {
    localStorage.removeItem('mokawa-recent-visits')
  } catch {
    notify('无法清理本机访问记录。')
  }
}
function openRecent(row: { file: CloudFile; visitedAt: string }) {
  if (isFolder(row.file)) {
    notify('本机记录未保存云端路径，请从“我的文件”重新定位此文件夹。')
    return
  }
  files.value = [row.file]
  selected.value = new Set([row.file.id])
  page.value = 'files'
  detail.value = true
}
async function copyShare(share: Share) {
  try {
    await navigator.clipboard.writeText(share.share_url)
    notify('分享链接已复制。')
  } catch {
    notify(`无法访问剪贴板。分享链接：${share.share_url}`)
  }
}
async function loadFavorites() {
  const generation = ++remotePageGeneration
  pageLoading.value = true
  remoteFiles.value = []
  remoteError.value = ''
  try {
    const result = await call<CloudFile[]>('list_favorites')
    if (generation === remotePageGeneration && page.value === 'favorites')
      remoteFiles.value = result
  } catch (error) {
    if (generation === remotePageGeneration && page.value === 'favorites') {
      remoteError.value = errorMessage(error)
      report(error)
    }
  } finally {
    if (generation === remotePageGeneration) pageLoading.value = false
  }
}
async function loadTrash() {
  const generation = ++remotePageGeneration
  pageLoading.value = true
  remoteFiles.value = []
  remoteError.value = ''
  try {
    const result = await call<CloudFile[]>('list_trash')
    if (generation === remotePageGeneration && page.value === 'trash') remoteFiles.value = result
  } catch (error) {
    if (generation === remotePageGeneration && page.value === 'trash') {
      remoteError.value = errorMessage(error)
      report(error)
    }
  } finally {
    if (generation === remotePageGeneration) pageLoading.value = false
  }
}
async function loadShares() {
  const generation = ++remotePageGeneration
  pageLoading.value = true
  shares.value = []
  remoteError.value = ''
  try {
    const result = await call<Share[]>('list_shares')
    if (generation === remotePageGeneration && page.value === 'shares') shares.value = result
  } catch (error) {
    if (generation === remotePageGeneration && page.value === 'shares') {
      remoteError.value = errorMessage(error)
      report(error)
    }
  } finally {
    if (generation === remotePageGeneration) pageLoading.value = false
  }
}
async function toggleFavorite(file: CloudFile) {
  try {
    const starred = !file.starred
    await call('set_favorite', { id: file.id, starred })
    files.value = files.value.map((row) => (row.id === file.id ? { ...row, starred } : row))
    remoteFiles.value = remoteFiles.value.map((row) =>
      row.id === file.id ? { ...row, starred } : row,
    )
    if (page.value === 'favorites') await loadFavorites()
    notify(starred ? '已加入 PikPak 收藏。' : '已从 PikPak 收藏移除。')
  } catch (error) {
    report(error)
  }
}
async function restore(file: CloudFile) {
  try {
    await call('restore_files', { ids: [file.id] })
    await loadTrash()
    notify('已从 PikPak 回收站恢复。')
  } catch (error) {
    report(error)
  }
}
async function permanentlyDelete(file: CloudFile) {
  if (!window.confirm(`永久删除“${file.name}”？此操作无法撤销。`)) return
  if (!window.confirm('再次确认：此文件将从 PikPak 永久删除。')) return
  try {
    await call('permanently_delete_files', { ids: [file.id], confirmed: true })
    await loadTrash()
    notify('文件已永久删除。')
  } catch (error) {
    report(error)
  }
}
async function deleteShare(share: Share) {
  if (!window.confirm(`取消分享“${share.title || '未命名分享'}”？`)) return
  try {
    await call('delete_shares', { ids: [share.share_id], confirmed: true })
    await loadShares()
    notify('分享已取消。')
  } catch (error) {
    report(error)
  }
}
function select(file: CloudFile, multi: boolean) {
  const next = multi ? new Set(selected.value) : new Set<string>()
  if (multi && next.has(file.id)) next.delete(file.id)
  else next.add(file.id)
  selected.value = next
}
function context(file: CloudFile, event: MouseEvent) {
  if (!selected.value.has(file.id)) select(file, false)
  menu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 195)),
    y: Math.max(48, Math.min(event.clientY, window.innerHeight - 245)),
  }
}
function report(error: unknown) {
  const message = errorMessage(error)
  const code = (error as AppError)?.code
  if (['unauthorized', 'no_session', 'invalid_session'].includes(code)) {
    authenticated.value = false
    if (code !== 'unauthorized') hasSavedSession.value = false
    quota.value = null
    quotaUpdatedAt.value = 0
    files.value = []
    remoteFiles.value = []
    shares.value = []
    offlineTasks.value = []
    path.value = []
    selected.value = new Set()
    detail.value = false
    dialog.value = null
    menu.value = null
    accountMenu.value = false
    page.value = 'login'
  }
  if ((error as AppError)?.code !== 'cancelled') {
    status.value = message
    notify(message)
  }
}
async function load() {
  const generation = ++requestGeneration
  files.value = []
  selected.value = new Set()
  busy.value = true
  status.value = '正在读取 PikPak 文件…'
  try {
    const result = await call<CloudFile[]>('list_files', {
      parent: path.value.at(-1)?.id || '',
      generation,
    })
    if (generation !== requestGeneration || disposed) return
    files.value = result
    selected.value = new Set()
    authenticated.value = true
    hasSavedSession.value = true
    page.value = 'files'
    status.value = `共 ${result.length.toLocaleString('zh-CN')} 项`
    void loadQuota()
  } catch (error) {
    if (generation === requestGeneration && !disposed) report(error)
  } finally {
    if (generation === requestGeneration) busy.value = false
  }
}
async function loadQuota(force = false) {
  if (!authenticated.value || quotaLoading.value) return
  if (!force && Date.now() - quotaUpdatedAt.value < 5 * 60 * 1000 && quota.value) return
  quotaLoading.value = true
  quotaError.value = false
  try {
    quota.value = await call<DriveQuota>('storage_quota')
    quotaUpdatedAt.value = Date.now()
  } catch (error) {
    quotaError.value = true
    if (['unauthorized', 'no_session', 'invalid_session'].includes((error as AppError)?.code))
      report(error)
  } finally {
    quotaLoading.value = false
  }
}
const quotaPercent = computed(() => {
  const { usage, limit } = quota.value || {}
  return usage != null && limit != null && limit > 0
    ? Math.min(100, Math.max(0, (usage / limit) * 100))
    : 0
})
function quotaLabel() {
  if (!authenticated.value) return '连接账号后查看容量'
  if (quotaLoading.value && !quota.value) return '正在读取容量…'
  if (quotaError.value && !quota.value) return '容量暂时不可用'
  if (quota?.value?.usage == null || quota.value.limit == null || quota.value.limit <= 0)
    return '容量信息不可用'
  return `${formatBytes(quota.value.usage)} / ${formatBytes(quota.value.limit)}`
}
function openFile(file: CloudFile) {
  recordVisit(file)
  if (isFolder(file)) {
    path.value.push({ id: file.id, name: file.name })
    query.value = ''
    void load()
  } else {
    select(file, false)
    detail.value = true
  }
}
function setMediaElement(element: unknown) {
  if (element instanceof HTMLMediaElement) mediaElement = element
}
function mediaFailure(kind: '视频' | '音频') {
  const code = mediaElement?.error?.code
  const reason =
    code === 2
      ? '读取网络失败，访问地址可能已过期或网络已中断。'
      : code === 3
        ? 'WebView2 无法解码此媒体编码。'
        : code === 4
          ? '访问地址无效或媒体格式不受支持。'
          : '媒体读取已中止。'
  previewError.value = `${kind}${reason}可重试或下载文件。`
}
function retryPreview() {
  detail.value = false
  void nextTick(() => {
    detail.value = true
  })
}
function breadcrumb(index: number) {
  path.value = path.value.slice(0, index + 1)
  query.value = ''
  void load()
}
async function connectPersonalAccessToken() {
  if (busy.value) return
  const token = personalAccessToken.value.trim()
  personalAccessToken.value = ''
  if (!token) {
    status.value = '请输入 PikPak 个人访问令牌。'
    return
  }
  busy.value = true
  status.value = '正在向 PikPak 验证令牌并读取根目录…'
  try {
    await call('connect_personal_access_token', { token })
    authMethod.value = 'personal_access_token'
    hasSavedSession.value = true
    path.value = []
    page.value = 'files'
    await load()
  } catch (error) {
    report(error)
  } finally {
    personalAccessToken.value = ''
    busy.value = false
  }
}
function cancelLogin() {
  requestGeneration++
  busy.value = false
  void call('cancel_page').catch(() => {})
  status.value = '已取消会话验证。'
}
async function signOut() {
  busy.value = true
  requestGeneration++
  try {
    await call('sign_out')
    authenticated.value = false
    authMethod.value = null
    hasSavedSession.value = false
    quota.value = null
    quotaUpdatedAt.value = 0
    files.value = []
    path.value = []
    selected.value = new Set()
    page.value = 'login'
    status.value = '本地会话已清除。'
  } catch (error) {
    report(error)
  } finally {
    busy.value = false
  }
}
async function edit(kind: 'folder' | 'rename' | 'trash') {
  menu.value = null
  if (kind !== 'folder' && !selected.value.size) {
    notify('请先选择文件。')
    return
  }
  if (kind === 'rename' && selected.value.size !== 1) {
    notify('请只选择一个文件。')
    return
  }
  returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  dialog.value = kind
  newName.value = kind === 'rename' ? currentFile.value?.name || '' : ''
  await nextTick()
  ;(nameInput.value || modal.value?.querySelector<HTMLButtonElement>('.cancel-action'))?.focus()
  nameInput.value?.select()
}
function createShare() {
  if (!selected.value.size) {
    notify('请先选择要分享的文件。')
    return
  }
  shareExpiration.value = 7
  sharePassword.value = true
  dialog.value = 'share'
}
watch(
  dialog,
  (value, previous) => {
    if (!value && previous && returnFocus?.isConnected) returnFocus.focus()
  },
  { flush: 'post' },
)
function trapDialog(event: KeyboardEvent) {
  if (event.key !== 'Tab' || !modal.value) return
  const elements = [
    ...modal.value.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled)'),
  ]
  const first = elements[0],
    last = elements.at(-1)
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last?.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first?.focus()
  }
}
async function confirm() {
  if (writing.value) return
  writing.value = true
  try {
    if (dialog.value === 'offline') {
      await call('create_offline_task', {
        source: offlineSource.value.trim(),
        parentId: offlineDestination.value,
      })
      offlineSource.value = ''
      dialog.value = null
      page.value = 'offline'
      await refreshOffline()
      notify('已向 PikPak 提交离线任务。')
      return
    } else if (dialog.value === 'folder')
      await call('create_folder', { parent: path.value.at(-1)?.id || '', name: newName.value })
    else if (dialog.value === 'rename')
      await call('rename_file', { id: currentFile.value?.id, name: newName.value })
    else if (dialog.value === 'share') {
      const share = await call<Share>('create_share', {
        ids: [...selected.value],
        expirationDays: shareExpiration.value,
        requirePassCode: sharePassword.value,
      })
      dialog.value = null
      if (share.share_url) {
        try {
          await navigator.clipboard.writeText(share.share_url)
        } catch {
          /* UI still presents the returned link below. */
        }
        notify(
          `分享已创建：${share.share_url}${share.pass_code ? ` 提取码：${share.pass_code}` : ''}`,
        )
      } else notify('PikPak 已处理分享请求，但未返回分享链接。')
      return
    } else await call('trash_files', { ids: [...selected.value], confirmed: true })
    dialog.value = null
    await load()
  } catch (error) {
    report(error)
  } finally {
    writing.value = false
  }
}
async function download() {
  menu.value = null
  const ids = [...selected.value].filter((id) => {
    const file = files.value.find((item) => item.id === id)
    return file && !isFolder(file)
  })
  if (!ids.length) {
    notify('请选择至少一个文件下载。')
    return
  }
  try {
    const task = await call<string | null>('start_download_batch', { ids })
    if (task) {
      page.value = 'tasks'
      notify('已加入下载队列。')
    }
  } catch (error) {
    report(error)
  }
}
async function refreshOffline() {
  if (!authenticated.value || offlineLoading.value) return
  offlineLoading.value = true
  offlineError.value = ''
  try {
    offlineTasks.value = await call<OfflineTask[]>('offline_tasks')
  } catch (error) {
    offlineError.value = errorMessage(error)
    report(error)
  } finally {
    offlineLoading.value = false
  }
}
function newOfflineTask() {
  if (!authenticated.value) {
    notify('连接 PikPak 后才能创建云端离线任务。')
    return
  }
  offlineDestination.value = path.value.at(-1)?.id || ''
  offlineSource.value = ''
  dialog.value = 'offline'
  void nextTick(() => modal.value?.querySelector<HTMLInputElement>('input')?.focus())
}
function selectTaskKind(kind: 'all' | 'local' | 'cloud') {
  taskKind.value = kind
  if (kind !== 'local') void refreshOffline()
}
async function importTorrent() {
  try {
    const magnet = await call<string | null>('choose_torrent')
    if (magnet) offlineSource.value = magnet
  } catch (error) {
    report(error)
  }
}
async function openOfflineResult(task: OfflineTask) {
  if (!task.file_id) {
    notify('PikPak 未返回完成文件 ID，请刷新任务或在“我的文件”中查看。')
    return
  }
  try {
    const result = await call<string | null>('start_download_batch', { ids: [task.file_id] })
    if (result) page.value = 'tasks'
  } catch (error) {
    report(error)
  }
}
async function retryLocalDownload(fileId: string) {
  try {
    const result = await call<string | null>('start_download_batch', { ids: [fileId] })
    if (result) notify('已重新加入下载队列。')
  } catch (error) {
    report(error)
  }
}
function offlinePhaseLabel(phase: string) {
  const labels: Record<string, string> = {
    PHASE_TYPE_PENDING: '等待云端处理',
    PHASE_TYPE_RUNNING: '云端下载中',
    PHASE_TYPE_COMPLETE: '已完成',
    PHASE_TYPE_ERROR: '失败',
    PHASE_TYPE_PAUSED: '已暂停',
    PHASE_TYPE_UNKNOW: '状态未知',
  }
  return labels[phase.toUpperCase()] || '状态未知'
}
async function removeOfflineTask(id: string) {
  if (!window.confirm('删除这条 PikPak 离线任务记录？已完成的云端文件会保留。')) return
  try {
    await call('delete_offline_task', { id })
    await refreshOffline()
    notify('离线任务记录已删除，云端文件保留。')
  } catch (error) {
    report(error)
  }
}
function unavailable(feature: string) {
  notify(`${feature}尚未接入，当前无法操作。`)
}
async function windowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
  if (!desktop) {
    notify('窗口控制仅在 Windows 桌面版本可用。')
    return
  }
  try {
    if (action === 'close') await appearance.flush()
    await getCurrentWindow()[action]()
  } catch (error) {
    notify(errorMessage(error))
  }
}
function titlebarPointerDown(event: PointerEvent) {
  if (!desktop || event.button !== 0) return
  titlebarPointer = { id: event.pointerId, x: event.clientX, y: event.clientY }
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
}
function titlebarPointerMove(event: PointerEvent) {
  const origin = titlebarPointer
  if (!origin || origin.id !== event.pointerId) return
  if (Math.hypot(event.clientX - origin.x, event.clientY - origin.y) < 5) return
  titlebarPointer = undefined
  void getCurrentWindow()
    .startDragging()
    .catch((error) => notify(errorMessage(error)))
}
function titlebarPointerEnd(event: PointerEvent) {
  if (titlebarPointer?.id === event.pointerId) titlebarPointer = undefined
}

function key(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    if (!writing.value) dialog.value = null
    menu.value = null
    accountMenu.value = false
  }
  if (event.key === 'F5') {
    event.preventDefault()
    if (
      authenticated.value &&
      ['files', 'home'].includes(page.value) &&
      !dialog.value &&
      !writing.value &&
      !busy.value
    )
      void load()
  }
}
let taskRevision = 0
onMounted(async () => {
  window.addEventListener('keydown', key)
  try {
    if (desktop) {
      const unlisten = await listen<Transfers>('transfers-changed', (event) => {
        taskRevision++
        tasks.value = event.payload
      })
      if (disposed) {
        unlisten()
        return
      }
      taskObserver = unlisten
    }
    const result = await call<Bootstrap>('bootstrap')
    if (disposed) return
    await appearance.initialize(result.appearance)
    if (disposed) return
    hasSavedSession.value = result.canClearSession
    authMethod.value = result.authMethod
    try {
      const saved = JSON.parse(localStorage.getItem('mokawa-recent-visits') || '[]')
      recentVisits.value = Array.isArray(saved)
        ? saved.filter((row) => row?.file?.id && row?.visitedAt).slice(0, 100)
        : []
    } catch {
      recentVisits.value = []
    }
    if (result.warning) status.value = result.warning
    const revision = taskRevision
    const initial = await call<Transfers>('transfer_snapshot')
    if (disposed) return
    if (revision === taskRevision) tasks.value = initial
    if (result.hasSession) void load()
    if (desktop) {
      const resize = await getCurrentWindow().onResized(async () => {
        const value = await getCurrentWindow().isMaximized()
        if (!disposed) maximized.value = value
      })
      if (disposed) {
        resize()
        return
      }
      resizedObserver = resize
      const close = await getCurrentWindow().onCloseRequested(async () => {
        closeFlush ??= appearance.flush()
        await closeFlush
      })
      if (disposed) {
        close()
        return
      }
      closeObserver = close
      maximized.value = await getCurrentWindow().isMaximized()
    }
  } catch (error) {
    report(error)
  } finally {
    booting.value = false
  }
})
onBeforeUnmount(() => {
  disposed = true
  personalAccessToken.value = ''
  requestGeneration++
  previewGeneration++
  void call('cancel_preview', { generation: previewGeneration }).catch(() => {})
  if (mediaElement) {
    mediaElement.pause()
    mediaElement.removeAttribute('src')
    mediaElement.load()
    mediaElement = undefined
  }
  window.removeEventListener('keydown', key)
  if (queryTimer) clearTimeout(queryTimer)
  if (noticeTimer) clearTimeout(noticeTimer)
  taskObserver?.()
  resizedObserver?.()
  closeObserver?.()
  void call('cancel_page').catch(() => {})
})
function closeMenus() {
  menu.value = null
  accountMenu.value = false
}
function showDetails() {
  if (currentFile.value) recordVisit(currentFile.value)
  detail.value = true
  menu.value = null
}
function showMoveUnavailable() {
  unavailable('移动与复制')
  menu.value = null
}
</script>
<template>
  <div
    class="app"
    :data-theme="dark ? 'dark' : 'light'"
    :class="{ 'reduce-motion': settings.reducedMotion, 'has-background': !!background }"
    :style="style"
    @click="closeMenus"
  >
    <Transition name="background-cross"
      ><div
        :key="background"
        class="background-layer"
        :style="{ backgroundImage: background ? `url(${background})` : 'none' }"
    /></Transition>
    <div class="background-overlay" />
    <header class="titlebar">
      <div
        class="titlebar-drag"
        @pointerdown="titlebarPointerDown"
        @pointermove="titlebarPointerMove"
        @pointerup="titlebarPointerEnd"
        @pointercancel="titlebarPointerEnd"
        @dblclick="windowAction('toggleMaximize')"
      />
      <span v-if="!desktop" class="preview-badge">浏览器预览 · 不连接网盘</span>
      <div class="window-controls">
        <button aria-label="最小化" @click="windowAction('minimize')"><Minus :size="15" /></button
        ><button
          :aria-label="maximized ? '还原窗口' : '最大化'"
          @click="windowAction('toggleMaximize')"
        >
          <Copy v-if="maximized" :size="13" /><Square v-else :size="13" /></button
        ><button class="close" aria-label="关闭窗口" @click="windowAction('close')">
          <X :size="17" />
        </button>
      </div>
    </header>
    <div v-if="booting" class="boot">
      <LoaderCircle class="spin" /><span>正在打开 Mokawa…</span>
    </div>
    <main
      v-else
      class="workspace"
      :class="{ 'sidebar-collapsed': sidebarCollapsed }"
      :inert="dialog !== null"
    >
      <aside v-if="page !== 'login'" class="sidebar panel">
        <div class="sidebar-topline">
          <span>工作空间</span
          ><button
            class="icon-button collapse-sidebar"
            aria-label="收起侧边栏"
            @click="sidebarCollapsed = true"
          >
            <PanelLeftClose :size="17" />
          </button>
        </div>
        <nav aria-label="主导航">
          <button
            v-for="item in navigation"
            :key="item.id"
            :class="{ active: page === item.id }"
            @click="route(item.id)"
          >
            <component :is="item.icon" :size="18" />{{ item.label }}
          </button>
          <div class="nav-caption">传输与应用</div>
          <button :class="{ active: page === 'tasks' }" @click="route('tasks')">
            <Download :size="18" />任务中心<span
              v-if="tasks.records.some((t) => t.phase === 'Running')"
              class="activity-dot"
            /></button
          ><button :class="{ active: page === 'settings' }" @click="route('settings')">
            <Settings2 :size="18" />设置
          </button>
        </nav>
        <div class="sidebar-footer">
          <div class="quota">
            <HardDrive :size="14" /><span class="quota-copy">
              <span>{{ quotaLabel() }}</span>
              <span
                v-if="
                  authenticated && quota?.usage != null && quota.limit != null && quota.limit > 0
                "
                class="quota-progress"
                aria-label="容量使用比例"
                ><span :style="{ width: `${quotaPercent}%` }"
              /></span>
            </span>
            <button
              v-if="authenticated"
              class="quota-refresh"
              aria-label="刷新容量信息"
              title="刷新容量信息"
              :disabled="quotaLoading"
              @click="loadQuota(true)"
            >
              <RefreshCw :size="12" :class="{ spin: quotaLoading }" />
            </button>
          </div>
          <button class="profile" @click.stop="accountMenu = !accountMenu">
            <span class="avatar"
              ><img v-if="avatar" :src="avatar" alt="Mokawa 本地头像" /><UserRound
                v-else
                :size="20" /></span
            ><span
              ><b>{{ authenticated ? '已连接 PikPak' : '未连接 PikPak' }}</b
              ><small>Mokawa 本地头像</small></span
            ><ChevronDown :size="14" />
          </button>
        </div>
      </aside>
      <button
        v-if="sidebarCollapsed && page !== 'login'"
        class="sidebar-reopen icon-button"
        aria-label="展开侧边栏"
        @click="sidebarCollapsed = false"
      >
        <PanelLeftOpen :size="18" />
      </button>
      <section v-if="page === 'login'" class="login-screen">
        <button class="login-settings" @click="route('settings')">
          <Settings2 :size="16" />外观设置
        </button>
        <div class="login-card panel">
          <i class="brand-mark hero">M</i>
          <h1>连接 PikPak 账号</h1>
          <p>连接后，在 Mokawa 管理你的 PikPak 文件。</p>
          <div class="auth-note">
            <CircleAlert :size="18" /><span
              >个人访问令牌<br /><small
                >令牌只会发往 PikPak 并在验证成功后存入 Windows 凭据管理器。官方将 PAT
                定位为个人脚本/NAS
                使用，并建议第三方应用采用自己的授权流程；请仅在你接受此限制时使用。</small
              ></span
            >
          </div>
          <label for="personal-access-token">PikPak 个人访问令牌</label>
          <input
            id="personal-access-token"
            v-model="personalAccessToken"
            type="password"
            autocomplete="off"
            spellcheck="false"
            :disabled="busy"
            placeholder="粘贴在 PikPak 创建的 PAT"
            @keydown.enter.prevent="connectPersonalAccessToken"
          />
          <button
            class="primary full"
            :disabled="busy || !personalAccessToken.trim()"
            @click="connectPersonalAccessToken"
          >
            <LoaderCircle v-if="busy" class="spin" :size="17" />{{
              busy ? '正在验证并读取文件…' : '验证并连接'
            }}
          </button>
          <small class="privacy"
            >验证请求只读取根目录一页；令牌仅在读取成功后保存。过期或撤销时需重新连接。</small
          >
          <button class="text-button" disabled title="等待 PikPak 为 Mokawa 明确桌面客户端授权契约">
            使用 PikPak 浏览器授权（尚未开放）
          </button>
          <div class="local-history-links">
            <button class="text-button" @click="route('recent')">
              <Clock3 :size="15" />查看本机最近访问
            </button>
            <button class="text-button" @click="route('tasks')">
              <Download :size="15" />查看本地下载记录
            </button>
          </div>
          <button :disabled="busy" @click="load">重试已保存的会话</button
          ><button v-if="busy" class="text-button" @click="cancelLogin">取消会话验证</button
          ><button v-if="hasSavedSession" :disabled="busy" @click="signOut">清除本地会话</button
          ><button class="text-button" @click="call('open_auth_help').catch(report)">
            查看 PikPak 官方授权说明
          </button>
          <p v-if="status" class="login-error" role="status">{{ status }}</p>
          <small class="privacy"
            >凭据仅由 Rust 与 Windows 凭据管理器处理，不交给页面，不发送至 Mokawa 服务器。</small
          >
        </div>
      </section>
      <section v-else-if="page === 'settings'" class="content-panel panel settings-page">
        <div class="settings-shell">
          <aside class="settings-nav">
            <h1>设置</h1>
            <p>管理 Mokawa 的外观与行为</p>
            <button
              v-for="item in settingsNavigation"
              :key="item.id"
              :class="{ active: settingsSection === item.id }"
              @click="settingsSection = item.id"
            >
              <component :is="item.icon" :size="17" />{{ item.label }}
            </button>
          </aside>
          <div class="settings-content">
            <AppearancePanel
              v-if="settingsSection === 'appearance'"
              :settings="settings"
              :avatar="avatar"
              :background="background"
              :save-status="saveStatus"
              :importing="importing"
              @choose="appearance.choose"
              @remove="appearance.remove"
              @reset="appearance.reset"
              @retry="appearance.flush"
            />
            <section v-else-if="settingsSection === 'downloads'" class="simple-settings">
              <p class="eyebrow">偏好设置</p>
              <h1>下载</h1>
              <div class="setting-section">
                <h2>下载目录</h2>
                <p>当前版本会在开始下载时由 Windows 选择保存位置。</p>
              </div>
              <div class="setting-section">
                <h2>并发数量</h2>
                <p>最多同时下载 2 个文件。任务队列会在应用关闭后保留状态。</p>
              </div>
              <div class="setting-section">
                <h2>下载行为</h2>
                <p>取消或失败的任务可从任务中心查看；暂停和续传能力尚未验证。</p>
              </div>
            </section>
            <section v-else-if="settingsSection === 'account'" class="simple-settings">
              <p class="eyebrow">PikPak 连接</p>
              <h1>账号</h1>
              <div class="account-setting-card">
                <span class="avatar"><UserRound :size="18" /></span>
                <div>
                  <b>{{ authenticated ? '已连接 PikPak' : '未连接 PikPak' }}</b>
                  <p>
                    {{
                      authenticated
                        ? authMethod === 'personal_access_token'
                          ? '个人访问令牌 · 已验证'
                          : 'OAuth 会话 · 已验证'
                        : authMethod
                          ? authMethod === 'personal_access_token'
                            ? 'PAT 已保存，等待重新验证'
                            : 'OAuth 会话待验证'
                          : '尚未连接 PikPak'
                    }}
                  </p>
                </div>
                <button v-if="authenticated" class="danger-text" @click="signOut">退出登录</button
                ><button v-else class="primary" @click="route('login')">前往连接</button>
              </div>
              <p class="setting-note">
                令牌由 Rust 后端验证并保存在 Windows 凭据管理器中。PikPak 尚未为 Mokawa
                提供已确认的浏览器授权流程。
              </p>
            </section>
            <section v-else class="simple-settings">
              <p class="eyebrow">应用信息</p>
              <h1>关于</h1>
              <div class="about-card">
                <i class="brand-mark hero">M</i>
                <h2>Mokawa</h2>
                <p>Windows PikPak 桌面客户端</p>
                <small>版本 {{ '0.1.0' }}</small>
              </div>
              <div class="setting-section">
                <h2>项目与更新</h2>
                <p>应用更新信息将在正式发布渠道开放后提供。</p>
              </div>
            </section>
          </div>
        </div>
      </section>
      <section v-else-if="page === 'offline'" class="content-panel panel tasks-page">
        <div class="page-heading">
          <div>
            <p class="eyebrow">PikPak 云端</p>
            <h1>离线下载</h1>
          </div>
          <div class="toolbar-actions">
            <button
              class="icon-button"
              :disabled="offlineLoading"
              aria-label="刷新离线任务"
              @click="refreshOffline"
            >
              <RefreshCw :size="16" :class="{ spin: offlineLoading }" /></button
            ><button class="primary" @click="newOfflineTask">
              <Plus :size="16" />新建离线任务
            </button>
          </div>
        </div>
        <p class="setting-note">
          任务由 PikPak 云端处理。支持 Magnet、HTTP/HTTPS 链接和 .torrent 种子。
        </p>
        <div v-if="offlineLoading && !offlineTasks.length" class="empty-state">
          <LoaderCircle class="spin" :size="26" />正在读取 PikPak 离线任务…
        </div>
        <div v-else-if="offlineError" class="empty-state">
          <CircleAlert :size="36" />
          <h2>读取云端任务失败</h2>
          <p>{{ offlineError }}</p>
          <button :disabled="offlineLoading" @click="refreshOffline">重试</button>
        </div>
        <div v-else-if="!offlineTasks.length" class="empty-state">
          <Download :size="36" />
          <h2>暂无云端离线任务</h2>
          <p>提交链接后，PikPak 会在云端获取文件。</p>
          <button class="primary" @click="newOfflineTask">新建离线任务</button>
        </div>
        <article v-for="task in offlineTasks" :key="task.id" class="transfer-row">
          <div class="file-type-icon"><Download :size="21" /></div>
          <div class="transfer-content">
            <b>{{ task.file_name || task.name || 'PikPak 离线任务' }}</b>
            <div class="transfer-meta">
              <span>{{ offlinePhaseLabel(task.phase) }}</span
              ><span v-if="task.file_size">{{ formatBytes(task.file_size) }}</span
              ><span v-if="task.updated_time">{{ formatDate(task.updated_time) }}</span>
            </div>
            <progress
              v-if="task.progress > 0 && task.progress <= 100"
              :value="task.progress"
              max="100"
            />
            <p v-if="task.message" class="error-text">{{ task.message }}</p>
          </div>
          <button
            v-if="['PHASE_TYPE_ERROR', 'ERROR', 'FAILED'].includes(task.phase.toUpperCase())"
            class="icon-button"
            aria-label="重试任务"
            title="重试"
            @click="call('retry_offline_task', { id: task.id }).then(refreshOffline).catch(report)"
          >
            <RefreshCw :size="16" />
          </button>
          <button
            v-if="
              ['PHASE_TYPE_COMPLETE', 'COMPLETE', 'COMPLETED'].includes(task.phase.toUpperCase())
            "
            class="icon-button"
            aria-label="下载完成文件"
            title="下载完成文件"
            @click="openOfflineResult(task)"
          >
            <Download :size="16" />
          </button>
          <button
            class="icon-button danger-text"
            aria-label="删除云端任务"
            title="删除任务（保留已完成文件）"
            @click="removeOfflineTask(task.id)"
          >
            <Trash2 :size="16" />
          </button>
        </article>
        <button
          v-if="authenticated"
          class="text-button"
          :disabled="offlineLoading"
          @click="refreshOffline"
        >
          刷新任务状态
        </button>
      </section>
      <section v-else-if="page === 'tasks'" class="content-panel panel tasks-page">
        <div class="page-heading">
          <div>
            <p class="eyebrow">传输记录</p>
            <h1>任务中心</h1>
          </div>
          <span class="muted">最多同时下载 2 个文件</span>
          <button
            v-if="taskKind !== 'local' && authenticated"
            class="icon-button"
            aria-label="刷新云端任务"
            :disabled="offlineLoading"
            @click="refreshOffline"
          >
            <RefreshCw :size="16" :class="{ spin: offlineLoading }" />
          </button>
        </div>
        <div class="task-filters">
          <div class="task-filter-group" aria-label="任务状态筛选">
            <span>状态</span>
            <button :class="{ active: taskFilter === 'all' }" @click="taskFilter = 'all'">
              全部
              <span>{{
                taskKind === 'cloud'
                  ? offlineTasks.length
                  : taskKind === 'local'
                    ? tasks.records.length
                    : tasks.records.length + offlineTasks.length
              }}</span></button
            ><button :class="{ active: taskFilter === 'active' }" @click="taskFilter = 'active'">
              进行中</button
            ><button
              :class="{ active: taskFilter === 'completed' }"
              @click="taskFilter = 'completed'"
            >
              已完成</button
            ><button :class="{ active: taskFilter === 'failed' }" @click="taskFilter = 'failed'">
              失败
            </button>
          </div>
          <div class="task-filter-group" aria-label="任务类型筛选">
            <span>类型</span>
            <button
              class="tasks-kind"
              :class="{ active: taskKind === 'all' }"
              @click="selectTaskKind('all')"
            >
              全部任务
            </button>
            <button
              class="tasks-kind"
              :class="{ active: taskKind === 'local' }"
              @click="selectTaskKind('local')"
            >
              本地下载
            </button>
            <button
              class="tasks-kind"
              :class="{ active: taskKind === 'cloud' }"
              @click="selectTaskKind('cloud')"
            >
              云端离线
            </button>
          </div>
        </div>
        <template v-if="taskKind !== 'local'">
          <h2 v-if="taskKind === 'all'" class="task-source-heading">云端离线</h2>
          <div v-if="!authenticated" class="empty-state">
            <CircleAlert :size="36" />
            <h2>连接 PikPak 后查看云端任务</h2>
            <button class="primary" @click="route('login')">连接账号</button>
          </div>
          <div v-else-if="offlineLoading && !offlineTasks.length" class="empty-state">
            <LoaderCircle class="spin" :size="32" />
            <h2>正在读取云端任务…</h2>
          </div>
          <div v-else-if="offlineError" class="empty-state">
            <CircleAlert :size="36" />
            <h2>读取云端任务失败</h2>
            <p>{{ offlineError }}</p>
            <button :disabled="offlineLoading" @click="refreshOffline">重试</button>
          </div>
          <div v-else-if="!filteredCloudTasks.length" class="empty-state">
            <Download :size="36" />
            <h2>{{ offlineTasks.length ? '没有符合筛选条件的任务' : '暂无云端离线任务' }}</h2>
            <button class="primary" @click="newOfflineTask">新建离线任务</button>
          </div>
          <article v-for="task in filteredCloudTasks" :key="task.id" class="transfer-row">
            <div class="file-type-icon"><Download :size="20" /></div>
            <div class="transfer-content">
              <b>{{ task.file_name || task.name || 'PikPak 离线任务' }}</b>
              <div class="transfer-meta">
                <span>{{ offlinePhaseLabel(task.phase) }}</span
                ><span v-if="task.file_size">{{ formatBytes(task.file_size) }}</span>
              </div>
              <progress
                v-if="task.progress > 0 && task.progress <= 100"
                :value="task.progress"
                max="100"
              />
              <p v-if="task.message" class="error-text">{{ task.message }}</p>
            </div>
            <button class="icon-button" aria-label="刷新离线任务" @click="refreshOffline">
              <RefreshCw :size="16" />
            </button>
          </article>
        </template>
        <template v-if="taskKind !== 'cloud'">
          <h2 v-if="taskKind === 'all'" class="task-source-heading">本地下载</h2>
          <div v-if="!filteredTasks.length" class="empty-state">
            <Download :size="38" />
            <h2>{{ tasks.records.length ? '没有符合筛选条件的任务' : '暂无本地下载记录' }}</h2>
            <p>本地任务记录可在未连接 PikPak 时查看；开始下载后会显示在这里。</p>
          </div>
          <div v-if="tasks.persistence_error" class="error-box">{{ tasks.persistence_error }}</div>
          <article v-for="task in filteredTasks" :key="task.id" class="transfer-row">
            <div class="file-type-icon"><Download :size="22" /></div>
            <div class="transfer-content">
              <b>{{ task.file_name }}</b>
              <div class="transfer-meta">
                <span
                  >{{ formatBytes(task.completed)
                  }}<template v-if="task.total"> / {{ formatBytes(task.total) }}</template></span
                ><span>{{ phases[task.phase] }}</span
                ><span v-if="tasks.speeds[task.id]"
                  >{{ formatBytes(tasks.speeds[task.id]) }}/秒</span
                >
              </div>
              <progress v-if="task.total" :value="task.completed" :max="task.total" />
              <p v-if="tasks.errors[task.id]" class="error-text">{{ tasks.errors[task.id] }}</p>
            </div>
            <button
              v-if="['Queued', 'Running'].includes(task.phase)"
              class="icon-button"
              aria-label="取消下载"
              @click="call('cancel_transfer', { id: task.id }).catch(report)"
            >
              <X :size="17" /></button
            ><button
              v-else-if="['Failed', 'Interrupted'].includes(task.phase) && task.file_id"
              class="icon-button"
              aria-label="重新下载"
              title="重新下载"
              @click="retryLocalDownload(task.file_id)"
            >
              <RefreshCw :size="16" /></button
            ><Check v-else-if="task.phase === 'Completed'" class="success" :size="20" />
          </article>
          <p class="footnote">取消与失败重试可用；暂停和断点续传尚未通过真实 PikPak 链接验证。</p>
        </template>
      </section>
      <section v-else-if="page === 'recent'" class="content-panel panel tasks-page">
        <div class="page-heading">
          <div>
            <p class="eyebrow">本机记录</p>
            <h1>最近访问</h1>
          </div>
          <button class="text-button" :disabled="!recentVisits.length" @click="clearRecent">
            清除记录
          </button>
        </div>
        <p class="setting-note">
          仅保存在此 Windows 用户的 Mokawa 本地设置中，不是 PikPak 云端访问历史。
        </p>
        <div v-if="!recentVisits.length" class="empty-state">
          <Clock3 :size="36" />
          <h2>暂无本机访问记录</h2>
          <p>打开文件或文件夹后，会在这里保留最近 100 项。</p>
        </div>
        <article
          v-for="row in recentVisits"
          :key="row.file.id"
          class="remote-file-row"
          @dblclick="openRecent(row)"
        >
          <Folder v-if="isFolder(row.file)" :size="18" /><FileText v-else :size="18" /><span
            class="remote-file-name"
            ><b>{{ row.file.name }}</b
            ><small>{{ formatDate(row.visitedAt) }} · 本机记录</small></span
          >
          <button
            v-if="!isFolder(row.file)"
            class="icon-button"
            title="预览详情"
            @click="openRecent(row)"
          >
            <FileText :size="16" />
          </button>
        </article>
      </section>
      <section v-else-if="page === 'favorites'" class="content-panel panel tasks-page">
        <div class="page-heading">
          <div>
            <p class="eyebrow">PikPak 云端</p>
            <h1>收藏</h1>
          </div>
          <button
            class="icon-button"
            aria-label="刷新收藏"
            :disabled="pageLoading"
            @click="loadFavorites"
          >
            <RefreshCw :size="16" :class="{ spin: pageLoading }" />
          </button>
        </div>
        <div v-if="pageLoading && !remoteFiles.length" class="empty-state">
          <LoaderCircle class="spin" />
          <h2>正在读取 PikPak 收藏…</h2>
        </div>
        <div v-else-if="remoteError" class="empty-state">
          <CircleAlert :size="36" />
          <h2>读取收藏失败</h2>
          <p>{{ remoteError }}</p>
          <button @click="loadFavorites">重试</button>
        </div>
        <div v-else-if="!remoteFiles.length" class="empty-state">
          <Star :size="36" />
          <h2>暂无收藏</h2>
          <p>收藏来自 PikPak 云端，文件列表中可添加或取消收藏。</p>
        </div>
        <article v-for="file in remoteFiles" :key="file.id" class="remote-file-row">
          <Folder v-if="isFolder(file)" :size="18" /><FileText v-else :size="18" /><span
            class="remote-file-name"
            ><b>{{ file.name }}</b
            ><small
              >{{ formatBytes(file.size) }} ·
              {{ file.modified_time ? formatDate(file.modified_time) : '时间未提供' }}</small
            ></span
          ><button
            class="icon-button"
            aria-label="取消收藏"
            title="从 PikPak 收藏移除"
            @click="toggleFavorite(file)"
          >
            <Star :size="16" />
          </button>
        </article>
      </section>
      <section v-else-if="page === 'trash'" class="content-panel panel tasks-page">
        <div class="page-heading">
          <div>
            <p class="eyebrow">PikPak 云端</p>
            <h1>回收站</h1>
          </div>
          <button
            class="icon-button"
            aria-label="刷新回收站"
            :disabled="pageLoading"
            @click="loadTrash"
          >
            <RefreshCw :size="16" :class="{ spin: pageLoading }" />
          </button>
        </div>
        <p class="setting-note">永久删除无法撤销。恢复和删除会直接修改 PikPak 云端内容。</p>
        <div v-if="pageLoading && !remoteFiles.length" class="empty-state">
          <LoaderCircle class="spin" />
          <h2>正在读取回收站…</h2>
        </div>
        <div v-else-if="remoteError" class="empty-state">
          <CircleAlert :size="36" />
          <h2>读取回收站失败</h2>
          <p>{{ remoteError }}</p>
          <button @click="loadTrash">重试</button>
        </div>
        <div v-else-if="!remoteFiles.length" class="empty-state">
          <Trash2 :size="36" />
          <h2>回收站为空</h2>
          <p>这里仅显示 PikPak 实际返回的已删除文件。</p>
        </div>
        <article v-for="file in remoteFiles" :key="file.id" class="remote-file-row">
          <Folder v-if="isFolder(file)" :size="18" /><FileText v-else :size="18" /><span
            class="remote-file-name"
            ><b>{{ file.name }}</b
            ><small
              >{{ formatBytes(file.size) }} ·
              {{ file.modified_time ? formatDate(file.modified_time) : '时间未提供' }}</small
            ></span
          ><button class="text-button" @click="restore(file)">恢复</button
          ><button
            class="icon-button danger-text"
            aria-label="永久删除"
            title="永久删除"
            @click="permanentlyDelete(file)"
          >
            <Trash2 :size="16" />
          </button>
        </article>
      </section>
      <section v-else-if="page === 'shares'" class="content-panel panel tasks-page">
        <div class="page-heading">
          <div>
            <p class="eyebrow">PikPak 云端</p>
            <h1>分享管理</h1>
          </div>
          <button
            class="icon-button"
            aria-label="刷新分享"
            :disabled="pageLoading"
            @click="loadShares"
          >
            <RefreshCw :size="16" :class="{ spin: pageLoading }" />
          </button>
        </div>
        <p class="setting-note">新建分享请先在“我的文件”选择文件，再使用工具栏中的分享操作。</p>
        <div v-if="pageLoading && !shares.length" class="empty-state">
          <LoaderCircle class="spin" />
          <h2>正在读取 PikPak 分享…</h2>
        </div>
        <div v-else-if="remoteError" class="empty-state">
          <CircleAlert :size="36" />
          <h2>读取分享失败</h2>
          <p>{{ remoteError }}</p>
          <button @click="loadShares">重试</button>
        </div>
        <div v-else-if="!shares.length" class="empty-state">
          <Share2 :size="36" />
          <h2>暂无分享</h2>
          <p>分享列表来自 PikPak 云端；可在文件列表中创建分享。</p>
          <button @click="route('files')">前往我的文件</button>
        </div>
        <article v-for="share in shares" :key="share.share_id" class="remote-file-row share-row">
          <Share2 :size="18" /><span class="remote-file-name"
            ><b>{{ share.title || 'PikPak 分享' }}</b
            ><small
              >{{ share.share_url }} ·
              {{ share.expiration_days < 0 ? '不限期' : `${share.expiration_days} 天` }} ·
              {{ share.file_num }} 个文件{{
                share.pass_code ? ` · 提取码 ${share.pass_code}` : ''
              }}</small
            ></span
          ><button
            class="icon-button"
            title="复制分享链接"
            aria-label="复制分享链接"
            @click="copyShare(share)"
          >
            <Copy :size="16" /></button
          ><button
            class="icon-button danger-text"
            title="取消分享"
            aria-label="取消分享"
            @click="deleteShare(share)"
          >
            <X :size="16" />
          </button>
        </article>
      </section>
      <section v-else-if="page === 'home'" class="content-panel panel home-page">
        <div class="home-topbar">
          <div class="search-field">
            <Search :size="18" /><input
              v-model="query"
              placeholder="搜索文件"
              aria-label="搜索文件"
            />
          </div>
          <button class="icon-button" aria-label="任务中心" @click="route('tasks')">
            <Download :size="18" />
          </button>
        </div>
        <div class="home-scroll">
          <div class="home-welcome">
            <div>
              <p class="eyebrow">MOKAWA · PIKPAK</p>
              <h1>你好，欢迎回来</h1>
              <p>
                {{
                  authenticated
                    ? '这里是你的云端文件空间。'
                    : '连接 PikPak 后，在这里管理云端文件。'
                }}
              </p>
            </div>
            <span class="connection-pill" :class="{ connected: authenticated }"
              ><i />{{ authenticated ? '已连接' : '未连接' }}</span
            >
          </div>
          <section class="home-section">
            <div class="section-heading">
              <div>
                <h2>常用入口</h2>
                <p>快速前往你的工作区域</p>
              </div>
            </div>
            <div class="quick-links">
              <button @click="route('files')">
                <Folder :size="19" /><span
                  ><b>我的文件</b
                  ><small>{{
                    authenticated ? `${files.length} 项已加载` : '连接后查看文件'
                  }}</small></span
                ><ChevronRight :size="16" /></button
              ><button @click="route('favorites')">
                <Star :size="19" /><span><b>收藏</b><small>查看已收藏的内容</small></span
                ><ChevronRight :size="16" /></button
              ><button @click="route('tasks')">
                <Download :size="19" /><span
                  ><b>任务中心</b
                  ><small>{{
                    tasks.records.length ? `${tasks.records.length} 个下载任务` : '查看下载任务'
                  }}</small></span
                ><ChevronRight :size="16" />
              </button>
            </div>
          </section>
          <section class="home-section">
            <div class="section-heading">
              <div>
                <h2>常用文件夹</h2>
                <p>
                  {{
                    authenticated ? '当前已加载目录中的真实文件夹' : '连接账号后显示真实云端目录'
                  }}
                </p>
              </div>
              <button v-if="authenticated" class="text-button" @click="route('files')">
                查看我的文件 <ChevronRight :size="14" />
              </button>
            </div>
            <div v-if="authenticated && commonFolders.length" class="home-recent">
              <button v-for="file in commonFolders" :key="file.id" @click="openFile(file)">
                <Folder :size="18" /><span
                  ><b>{{ file.name }}</b
                  ><small
                    >文件夹 · {{ file.modified_time ? formatDate(file.modified_time) : '—' }}</small
                  ></span
                ><ChevronRight :size="15" />
              </button>
            </div>
            <div v-else class="home-empty">
              <Folder :size="20" /><span>{{
                authenticated
                  ? '当前目录没有文件夹。最近访问记录尚未由当前 PikPak 接口提供。'
                  : '连接 PikPak 后，真实文件夹会显示在这里。'
              }}</span
              ><button v-if="!authenticated" class="primary" @click="route('login')">
                连接 PikPak
              </button>
            </div>
          </section>
          <section class="home-section">
            <div class="section-heading">
              <div>
                <h2>最近下载</h2>
                <p>任务状态来自本机下载队列</p>
              </div>
              <button v-if="recentTasks.length" class="text-button" @click="route('tasks')">
                打开任务中心 <ChevronRight :size="14" />
              </button>
            </div>
            <div v-if="recentTasks.length" class="home-recent">
              <div v-for="task in recentTasks" :key="task.id">
                <Download :size="18" /><span
                  ><b>{{ task.file_name }}</b
                  ><small
                    >{{ phases[task.phase] }} · {{ formatBytes(task.completed)
                    }}{{ task.total ? ` / ${formatBytes(task.total)}` : '' }}</small
                  ></span
                >
              </div>
            </div>
            <div v-else class="home-empty"><Download :size="20" /><span>暂无下载记录</span></div>
          </section>
        </div>
      </section>
      <section v-else-if="page === 'files'" class="content-panel panel files-page">
        <div class="files-topbar">
          <div class="search-field">
            <Search :size="18" /><input
              v-model="query"
              placeholder="搜索已加载的文件"
              aria-label="搜索当前文件夹"
            /><kbd>当前目录</kbd>
          </div>
          <button class="icon-button" aria-label="任务中心" @click="route('tasks')">
            <Download :size="19" /></button
          ><button class="avatar" aria-label="账号菜单" @click.stop="accountMenu = !accountMenu">
            <img v-if="avatar" :src="avatar" alt="Mokawa 本地头像" /><UserRound v-else :size="19" />
          </button>
        </div>
        <div class="file-heading">
          <div class="breadcrumbs">
            <button
              class="icon-button"
              aria-label="返回上级文件夹"
              :disabled="!path.length || busy"
              @click="breadcrumb(path.length - 2)"
            >
              <ArrowLeft :size="17" /></button
            ><button @click="breadcrumb(-1)">我的文件</button
            ><template v-for="(part, index) in path" :key="part.id"
              ><ChevronRight :size="14" /><button @click="breadcrumb(index)">
                {{ part.name }}
              </button></template
            >
          </div>
          <button class="icon-button" aria-label="刷新文件" :disabled="busy" @click="load">
            <RefreshCw :size="16" :class="{ spin: busy }" />
          </button>
        </div>
        <div class="file-toolbar">
          <button class="primary" :disabled="busy" @click="edit('folder')">
            <Plus :size="17" />新建文件夹</button
          ><button @click="unavailable('上传')"><Upload :size="16" />上传文件</button
          ><button class="offline-create" @click="newOfflineTask">
            <Link2 :size="16" />新建离线任务
          </button>
          <div class="toolbar-spacer" />
          <button @click="descending = !descending">
            名称{{ descending ? '降序' : '升序' }}<ChevronDown :size="14" />
          </button>
          <div class="segmented">
            <button aria-label="列表视图" :class="{ active: !grid }" @click="grid = false">
              <List :size="17" /></button
            ><button aria-label="网格视图" :class="{ active: grid }" @click="grid = true">
              <Grid2X2 :size="16" />
            </button>
          </div>
          <button class="icon-button" aria-label="显示文件详情" @click="detail = !detail">
            <Ellipsis :size="19" />
          </button>
        </div>
        <div v-if="selected.size" class="selection-toolbar">
          <b>已选择 {{ selected.size }} 项</b
          ><button @click="download"><Download :size="15" />下载</button
          ><button @click="edit('rename')">重命名</button
          ><button @click="createShare"><Share2 :size="15" />分享</button
          ><button class="danger-text" @click="edit('trash')">移入回收站</button
          ><button class="text-button" @click="selected = new Set()">取消选择</button>
        </div>
        <div v-if="!grid && visible.length" class="file-table-head">
          <span>文件名称</span><span>大小</span><span>修改时间</span>
        </div>
        <VirtualFiles
          v-if="visible.length"
          :files="visible"
          :grid="grid"
          :selected="selected"
          @select="select"
          @open="openFile"
          @menu="context"
        />
        <div v-else class="empty-state">
          <LoaderCircle v-if="busy || indexing" class="spin" :size="36" /><Folder
            v-else
            :size="40"
          />
          <h2>
            {{ busy || indexing ? '正在加载文件…' : query ? '没有匹配的文件' : '这里还没有文件' }}
          </h2>
          <p>
            {{
              busy || indexing
                ? '正在准备文件列表，请稍候。'
                : query
                  ? '换个关键词再试试。'
                  : '刷新文件列表，或新建一个文件夹。'
            }}
          </p>
          <button v-if="!busy" @click="load">重新加载</button>
        </div>
        <footer class="files-status">
          {{ status || `${visible.length} 项` }}<span>文件数据来自 PikPak</span>
        </footer>
      </section>
      <section v-else class="content-panel panel unavailable-page">
        <h1>{{ navigation.find((item) => item.id === page)?.label }}</h1>
        <div class="empty-state">
          <CircleAlert :size="36" />
          <h2>此功能尚未接入</h2>
          <p>仅在真实 API 能力确认后开放，不展示模拟数据。</p>
          <button @click="route('files')">返回我的文件</button>
        </div>
      </section>
      <aside v-if="detail && currentFile && page === 'files'" class="detail-panel panel">
        <div class="detail-heading">
          <h2>文件详情</h2>
          <button class="icon-button" aria-label="关闭文件详情" @click="detail = false">
            <X :size="18" />
          </button>
        </div>
        <div class="detail-preview" :key="currentFile.id">
          <LoaderCircle v-if="previewLoading" class="spin" :size="28" />
          <img
            v-else-if="previewKind === 'image' && previewUrl && !previewError"
            :src="previewUrl"
            :alt="currentFile.name"
            referrerpolicy="no-referrer"
            @error="previewError = '图片无法读取，链接可能已过期或文件格式不受支持。'"
          />
          <video
            v-else-if="previewKind === 'video' && previewUrl && !previewError"
            :ref="setMediaElement"
            :src="previewUrl"
            controls
            playsinline
            preload="metadata"
            referrerpolicy="no-referrer"
            @error="mediaFailure('视频')"
          />
          <audio
            v-else-if="previewKind === 'audio' && previewUrl && !previewError"
            :ref="setMediaElement"
            :src="previewUrl"
            controls
            preload="metadata"
            referrerpolicy="no-referrer"
            @error="mediaFailure('音频')"
          />
          <pre v-else-if="previewKind === 'text' && !previewError" class="text-preview">{{
            textContent
          }}</pre>
          <div v-else-if="previewError" class="preview-message" role="status">
            {{ previewError }}
          </div>
          <template v-else-if="previewKind === 'other'"
            ><Folder v-if="isFolder(currentFile)" :size="42" /><FileText v-else :size="42"
          /></template>
          <div v-else class="preview-message">正在准备预览…</div>
        </div>
        <h3 :title="currentFile.name">{{ currentFile.name }}</h3>
        <dl>
          <dt>类型</dt>
          <dd>
            {{
              isFolder(currentFile)
                ? '文件夹'
                : currentFile.mime_type ||
                  currentFile.name.split('.').at(-1)?.toUpperCase() ||
                  '未知文件'
            }}
          </dd>
          <dt>大小</dt>
          <dd>{{ formatBytes(currentFile.size) }}</dd>
          <dt>修改时间</dt>
          <dd>
            <time :datetime="currentFile.modified_time">{{
              currentFile.modified_time ? formatDate(currentFile.modified_time) : '服务端未提供'
            }}</time>
          </dd>
        </dl>
        <button v-if="!isFolder(currentFile)" @click="download"><Download :size="16" />下载</button>
        <button v-if="previewError" class="text-button" @click="retryPreview">重试预览</button>
        <p v-if="previewKind === 'other' && !isFolder(currentFile)" class="footnote">
          此文件类型暂不支持预览，可下载到本机查看。
        </p>
        <p v-if="previewKind === 'text' && !previewError" class="footnote">
          只读文本预览，最多读取 512 KB；内容由 PikPak 返回。
        </p>
      </aside>
    </main>
    <div v-if="accountMenu" class="account-popover popup" @click.stop>
      <div class="account-summary">
        <span class="avatar large"
          ><img v-if="avatar" :src="avatar" alt="本地头像" /><UserRound v-else :size="25" /></span
        ><b>{{ authenticated ? '已连接 PikPak' : '尚未登录' }}</b
        ><small>{{
          authMethod === 'personal_access_token'
            ? '认证方式：个人访问令牌'
            : authMethod === 'oauth'
              ? '认证方式：OAuth'
              : '头像仅用于 Mokawa 本地界面'
        }}</small>
      </div>
      <button @click="route('settings')"><Settings2 :size="16" />个性化设置</button
      ><button v-if="authenticated" @click="signOut"><LogOut :size="16" />退出登录</button
      ><button v-else @click="route('login')">连接 PikPak 账号</button>
    </div>
    <div
      v-if="menu"
      class="file-context popup"
      :style="{ left: `${menu.x}px`, top: `${menu.y}px` }"
      @click.stop
    >
      <button @click="download"><Download :size="16" />下载</button
      ><button @click="edit('rename')">重命名</button><button @click="showDetails">文件详情</button
      ><button @click="currentFile && toggleFavorite(currentFile)">
        <Star :size="15" />{{ currentFile?.starred ? '取消收藏' : '加入收藏' }}</button
      ><button @click="showMoveUnavailable">移动与复制（尚未接入）</button
      ><button class="danger-text" @click="edit('trash')"><Trash2 :size="16" />移入回收站</button>
    </div>
    <div v-if="dialog" class="modal-backdrop" @click.self="!writing && (dialog = null)">
      <form
        ref="modal"
        class="modal popup"
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-title"
        @keydown="trapDialog"
        @submit.prevent="confirm"
      >
        <div class="detail-heading">
          <h2 id="modal-title">
            {{
              dialog === 'offline'
                ? '新建 PikPak 离线任务'
                : dialog === 'share'
                  ? '创建 PikPak 分享'
                  : dialog === 'folder'
                    ? '新建文件夹'
                    : dialog === 'rename'
                      ? '重命名'
                      : '移入回收站'
            }}
          </h2>
          <button
            type="button"
            class="icon-button"
            :disabled="writing"
            aria-label="关闭对话框"
            @click="dialog = null"
          >
            <X :size="17" />
          </button>
        </div>
        <template v-if="dialog === 'offline'">
          <label
            >Magnet 或 HTTP/HTTPS 链接<textarea
              v-model="offlineSource"
              required
              maxlength="8192"
              rows="3"
              placeholder="粘贴 magnet:?xt=... 或 https://…"
              :disabled="writing"
            />
          </label>
          <button type="button" class="text-button" :disabled="writing" @click="importTorrent">
            <Upload :size="15" />导入 .torrent 种子文件
          </button>
          <label
            >云端保存目录<select v-model="offlineDestination" :disabled="writing">
              <option value="">PikPak 离线下载目录</option>
              <option v-if="path.length" :value="path.at(-1)?.id">
                当前目录：{{ path.map((part) => part.name).join(' / ') }}
              </option>
              <option v-for="folder in files.filter(isFolder)" :key="folder.id" :value="folder.id">
                {{ folder.name }}
              </option>
            </select></label
          >
          <p class="muted">
            种子文件仅在本机解析 info hash，Mokawa 不会上传种子文件，也不运行 BT 引擎。最终处理由
            PikPak 云端 API 完成。
          </p>
        </template>
        <template v-else-if="dialog === 'share'">
          <label
            >有效期<select v-model.number="shareExpiration" :disabled="writing">
              <option :value="1">1 天</option>
              <option :value="7">7 天</option>
              <option :value="30">30 天</option>
            </select></label
          >
          <label class="check-setting"
            ><input
              v-model="sharePassword"
              type="checkbox"
              :disabled="writing"
            />要求访问提取码</label
          >
          <p class="muted">
            分享由 PikPak 云端创建。若 API 不支持当前 PAT 权限，Mokawa
            会保留账号连接并显示权限错误。
          </p>
        </template>
        <template v-else-if="dialog === 'trash'"
          ><p>确定将选中的 {{ selected.size }} 项移入 PikPak 回收站？</p>
          <p class="muted">操作将修改真实网盘文件，请确认选择无误。</p></template
        ><label v-else
          >名称<input
            ref="nameInput"
            v-model="newName"
            maxlength="255"
            required
            :disabled="writing"
            aria-label="文件或文件夹名称"
        /></label>
        <div class="modal-actions">
          <button type="button" class="cancel-action" :disabled="writing" @click="dialog = null">
            取消</button
          ><button type="submit" class="primary" :disabled="writing">
            {{ writing ? '正在处理…' : '确认' }}
          </button>
        </div>
      </form>
    </div>
    <div v-if="notice" class="toast" role="status">
      <CircleAlert :size="17" /><span>{{ notice }}</span
      ><button class="icon-button" aria-label="关闭提示" @click="notice = ''">
        <X :size="15" />
      </button>
    </div>
  </div>
</template>
