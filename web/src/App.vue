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
const maximized = ref(false)
const navigation = [
  { id: 'home', label: '首页', icon: Home },
  { id: 'files', label: '我的文件', icon: Folder },
  { id: 'recent', label: '最近访问', icon: Clock3 },
  { id: 'favorites', label: '收藏', icon: Star },
  { id: 'offline', label: '离线下载', icon: Download },
  { id: 'shares', label: '分享管理', icon: Share2 },
  { id: 'trash', label: '回收站', icon: Trash2 },
]
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
async function authorize() {
  if (busy.value) return
  busy.value = true
  status.value = '正在检查授权方式…'
  try {
    await call('begin_authorization')
  } catch (error) {
    report(error)
  } finally {
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
      <div class="titlebar-drag" data-tauri-drag-region="deep">
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
    <main v-else class="workspace" :inert="dialog !== null">
      <aside v-if="page !== 'login'" class="sidebar panel">
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
      <section v-if="page === 'login'" class="login-screen">
        <button class="login-settings" @click="route('settings')">
          <Settings2 :size="16" />外观设置
        </button>
        <div class="login-card panel">
          <i class="brand-mark hero">M</i>
          <h1>连接 PikPak 账号</h1>
          <p>授权后，在 Mokawa 管理你的网盘文件。</p>
          <button class="primary full" :disabled="busy" @click="authorize">
            <LoaderCircle v-if="busy" class="spin" :size="17" />{{
              busy ? '正在验证…' : '使用 PikPak 授权登录'
            }}
          </button>
          <div class="auth-note">
            <CircleAlert :size="18" /><span
              >授权尚未开放<br /><small
                >Mokawa
                专用客户端注册与回调规则仍待确认。当前无法新增登录，不提供未经确认的密码登录。</small
              ></span
            >
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
        <AppearancePanel
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
          <button class="active">
            下载任务 <span>{{ tasks.records.length }}</span></button
          ><button @click="unavailable('上传')">上传任务</button
          ><button @click="unavailable('离线下载')">离线任务</button>
        </div>
        <div v-if="!tasks.records.length" class="empty-state">
          <Download :size="38" />
          <h2>暂无下载任务</h2>
          <p>登录后，选择文件并点击下载。</p>
        </div>
        <div v-if="tasks.persistence_error" class="error-box">{{ tasks.persistence_error }}</div>
        <article v-for="task in [...tasks.records].reverse()" :key="task.id" class="transfer-row">
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
      <section
        v-else-if="page === 'files' || page === 'home'"
        class="content-panel panel files-page"
      >
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
