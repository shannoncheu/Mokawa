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
  PanelLeftClose,
  PanelLeftOpen,
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
const busy = ref(false)
const personalAccessToken = ref('')
const booting = ref(true)
const status = ref('')
const files = shallowRef<CloudFile[]>([])
const selected = shallowRef(new Set<string>())
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
const dialog = ref<'folder' | 'rename' | 'trash' | null>(null)
const newName = ref('')
const writing = ref(false)
const nameInput = ref<HTMLInputElement>()
let requestGeneration = Date.now()
let disposed = false
let queryTimer: ReturnType<typeof setTimeout> | undefined
let resizedObserver: UnlistenFn | undefined
let taskObserver: UnlistenFn | undefined
let closeObserver: UnlistenFn | undefined
let closeFlush: Promise<void> | undefined
let titlebarPointer: { id: number; x: number; y: number } | undefined
const maximized = ref(false)
const sidebarCollapsed = ref(false)
const settingsSection = ref('appearance')
const taskFilter = ref<'all' | 'active' | 'completed' | 'failed'>('all')
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
const recentTasks = computed(() => [...tasks.value.records].reverse().slice(0, 3))
const commonFolders = computed(() => files.value.filter(isFolder).slice(0, 6))
const { visible, indexing } = useFileView(files, debouncedQuery, descending, notify)
const currentFile = computed(() =>
  selected.value.size === 1 ? files.value.find((file) => selected.value.has(file.id)) : undefined,
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
  if (['settings', 'tasks', 'login'].includes(id)) {
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
  if ((error as AppError)?.code === 'authentication') {
    authenticated.value = false
    files.value = []
    selected.value = new Set()
    path.value = []
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
    page.value = 'files'
    status.value = `共 ${result.length.toLocaleString('zh-CN')} 项`
  } catch (error) {
    if (generation === requestGeneration && !disposed) report(error)
  } finally {
    if (generation === requestGeneration) busy.value = false
  }
}
function openFile(file: CloudFile) {
  if (isFolder(file)) {
    path.value.push({ id: file.id, name: file.name })
    query.value = ''
    void load()
  } else {
    select(file, false)
    detail.value = true
  }
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
    hasSavedSession.value = false
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
    if (dialog.value === 'folder')
      await call('create_folder', { parent: path.value.at(-1)?.id || '', name: newName.value })
    else if (dialog.value === 'rename')
      await call('rename_file', { id: currentFile.value?.id, name: newName.value })
    else await call('trash_files', { ids: [...selected.value], confirmed: true })
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
  if (!currentFile.value || isFolder(currentFile.value)) {
    notify('请选择一个文件下载。')
    return
  }
  try {
    const task = await call<string | null>('start_download', { id: currentFile.value.id })
    if (task) {
      page.value = 'tasks'
      notify('已加入下载队列。')
    }
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
    :class="{ 'reduce-motion': settings.reducedMotion }"
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
      >
        <i class="brand-mark">M</i><b>Mokawa</b>
      </div>
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
            <HardDrive :size="14" /><span>{{
              authenticated ? '容量信息尚未接入' : '连接账号后查看容量'
            }}</span>
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
                  <p>{{ authenticated ? '当前会话已验证' : '使用个人访问令牌连接' }}</p>
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
      <section v-else-if="page === 'tasks'" class="content-panel panel tasks-page">
        <div class="page-heading">
          <div>
            <p class="eyebrow">传输记录</p>
            <h1>任务中心</h1>
          </div>
          <span class="muted">最多同时下载 2 个文件</span>
        </div>
        <div class="tabs">
          <button :class="{ active: taskFilter === 'all' }" @click="taskFilter = 'all'">
            全部 <span>{{ tasks.records.length }}</span></button
          ><button :class="{ active: taskFilter === 'active' }" @click="taskFilter = 'active'">
            进行中</button
          ><button
            :class="{ active: taskFilter === 'completed' }"
            @click="taskFilter = 'completed'"
          >
            已完成</button
          ><button :class="{ active: taskFilter === 'failed' }" @click="taskFilter = 'failed'">
            失败</button
          ><span class="tasks-kind">下载任务</span>
        </div>
        <div v-if="!filteredTasks.length" class="empty-state">
          <Download :size="38" />
          <h2>暂无下载任务</h2>
          <p>登录后，选择文件并点击下载。</p>
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
              ><span v-if="tasks.speeds[task.id]">{{ formatBytes(tasks.speeds[task.id]) }}/秒</span>
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
          ><Check v-else-if="task.phase === 'Completed'" class="success" :size="20" />
        </article>
        <p class="footnote">尚未验证暂停与续传能力。上传、离线下载尚未接入。</p>
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
                    >文件夹 ·
                    {{
                      file.modified_time
                        ? new Date(file.modified_time).toLocaleDateString('zh-CN')
                        : '—'
                    }}</small
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
          ><button @click="unavailable('上传')"><Upload :size="16" />上传文件</button>
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
        <div class="detail-preview"><Folder :size="52" /></div>
        <h3>{{ currentFile.name }}</h3>
        <dl>
          <dt>类型</dt>
          <dd>{{ isFolder(currentFile) ? '文件夹' : currentFile.mime_type || '服务端未提供' }}</dd>
          <dt>大小</dt>
          <dd>{{ formatBytes(currentFile.size) }}</dd>
          <dt>修改时间</dt>
          <dd>{{ currentFile.modified_time || '服务端未提供' }}</dd>
        </dl>
        <button @click="download"><Download :size="16" />下载</button>
        <p class="footnote">媒体预览尚未接入。</p>
      </aside>
    </main>
    <div v-if="accountMenu" class="account-popover popup" @click.stop>
      <div class="account-summary">
        <span class="avatar large"
          ><img v-if="avatar" :src="avatar" alt="本地头像" /><UserRound v-else :size="25" /></span
        ><b>{{ authenticated ? '已连接 PikPak' : '尚未登录' }}</b
        ><small>头像仅用于 Mokawa 本地界面</small>
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
            {{ dialog === 'folder' ? '新建文件夹' : dialog === 'rename' ? '重命名' : '移入回收站' }}
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
        <template v-if="dialog === 'trash'"
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
