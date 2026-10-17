import { mount, flushPromises } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import App from '../src/App.vue'
import { defaults } from '../src/types'
const transport = vi.hoisted(() => ({ call: vi.fn() }))
vi.mock('../src/useFileView', () => ({
  useFileView: (files: { value: unknown[] }) => ({ visible: files, indexing: false }),
}))
vi.mock('../src/bridge', () => ({
  desktop: false,
  call: (...args: unknown[]) => transport.call(...args),
  errorMessage: (error: { message?: string }) => error.message || '错误',
}))
beforeEach(() => {
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    },
  )
  transport.call.mockReset().mockImplementation(async (command: string) => {
    if (command === 'bootstrap')
      return {
        appearance: { settings: defaults(), avatar: null, background: null, warnings: [] },
        hasSession: true,
        canClearSession: true,
        authMethod: 'personal_access_token',
        warning: null,
      }
    if (command === 'transfer_snapshot')
      return { records: [], errors: {}, speeds: {}, persistence_error: null }
    if (command === 'list_files')
      return [
        {
          id: 'file-a',
          name: '真实协议的测试替身.txt',
          kind: 'drive#file',
          size: 42,
          mime_type: 'text/plain',
          modified_time: '',
        },
      ]
  })
})
it('只有会话文件请求成功后进入文件页，删除必须经确认', async () => {
  const wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  expect(wrapper.find('.files-page').exists()).toBe(true)
  await wrapper.find('.selection-box').trigger('click')
  const trash = wrapper
    .findAll('.selection-toolbar button')
    .find((button) => button.text() === '移入回收站')!
  await trash.trigger('click')
  await flushPromises()
  expect(wrapper.find('[role="dialog"]').exists()).toBe(true)
  expect(transport.call.mock.calls.some((args) => args[0] === 'trash_files')).toBe(false)
  await wrapper.find('form.modal').trigger('submit')
  await flushPromises()
  expect(transport.call).toHaveBeenCalledWith('trash_files', { ids: ['file-a'], confirmed: true })
  wrapper.unmount()
})
it('点击文件主体打开详情，复选框独立控制选择和批量工具栏', async () => {
  const wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  await wrapper.find('.file-item').trigger('click')
  await flushPromises()
  expect(wrapper.find('.detail-panel').exists()).toBe(true)
  expect(wrapper.find('.selection-toolbar').exists()).toBe(false)
  await wrapper.find('[aria-label="关闭文件详情"]').trigger('click')
  await flushPromises()
  await wrapper.find('.selection-box').trigger('click')
  expect(wrapper.find('.selection-toolbar').text()).toContain('已选择 1 项')
  await wrapper.find('.selection-box').trigger('click')
  expect(wrapper.find('.selection-toolbar').exists()).toBe(false)
  wrapper.unmount()
})
it('右键菜单提供真实文件操作和媒体预览，不显示未接入的移动操作', async () => {
  const wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  await wrapper.find('.file-item').trigger('contextmenu', { clientX: 100, clientY: 100 })
  await flushPromises()
  const menu = wrapper.find('[role="menu"]')
  expect(menu.exists()).toBe(true)
  expect(menu.text()).toContain('预览文本')
  expect(menu.text()).toContain('下载')
  expect(menu.text()).toContain('加入收藏')
  expect(menu.text()).not.toContain('移动与复制')
  await menu.find('[role="menuitem"]').trigger('click')
  await flushPromises()
  expect(wrapper.find('.detail-panel').exists()).toBe(true)
  wrapper.unmount()
})
it('无效保存会话不展示文件主页，也不展示退出登录', async () => {
  const original = transport.call.getMockImplementation()!
  transport.call.mockImplementation(async (command: string, ...args: unknown[]) => {
    if (command === 'list_files')
      throw { code: 'unauthorized', message: '会话已过期，请重新登录。' }
    return original(command, ...args)
  })
  const wrapper = mount(App)
  await flushPromises()
  expect(wrapper.find('.login-screen').exists()).toBe(true)
  expect(wrapper.find('.files-page').exists()).toBe(false)
  expect(wrapper.text()).toContain('会话已过期')
  expect(wrapper.findAll('button').some((button) => button.text() === '退出登录')).toBe(false)
  wrapper.unmount()
})
it('写操作失去认证时关闭旧对话框并返回登录页', async () => {
  const original = transport.call.getMockImplementation()!
  transport.call.mockImplementation(async (command: string, ...args: unknown[]) => {
    if (command === 'rename_file') throw { code: 'unauthorized', message: '会话已过期' }
    return original(command, ...args)
  })
  const wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  await wrapper.find('.selection-box').trigger('click')
  await wrapper
    .findAll('.selection-toolbar button')
    .find((button) => button.text() === '重命名')!
    .trigger('click')
  await flushPromises()
  await wrapper.find('form.modal').trigger('submit')
  await flushPromises()
  expect(wrapper.find('.login-screen').exists()).toBe(true)
  expect(wrapper.find('[role="dialog"]').exists()).toBe(false)
  wrapper.unmount()
})
it('切换目录期间不显示或操作上个目录的文件', async () => {
  const original = transport.call.getMockImplementation()!
  let calls = 0
  let complete!: (files: unknown[]) => void
  transport.call.mockImplementation(async (command: string, ...args: unknown[]) => {
    if (command === 'list_files') {
      if (++calls === 1)
        return [
          {
            id: 'folder-a',
            name: '目录',
            kind: 'drive#folder',
            mime_type: '',
            modified_time: '',
            size: 0,
          },
        ]
      return new Promise((resolve) => {
        complete = resolve
      })
    }
    return original(command, ...args)
  })
  const wrapper = mount(App)
  await flushPromises()
  await wrapper.find('.file-item').trigger('click')
  await flushPromises()
  expect(wrapper.findAll('.file-item')).toHaveLength(0)
  expect(wrapper.text()).toContain('正在加载文件')
  expect(wrapper.find('.selection-toolbar').exists()).toBe(false)
  complete([])
  await flushPromises()
  expect(wrapper.text()).toContain('这里还没有文件')
  wrapper.unmount()
})
it('未连接时仍可查看本地下载，状态筛选互斥且不显示云端任务', async () => {
  transport.call.mockImplementation(async (command: string) => {
    if (command === 'bootstrap')
      return {
        appearance: { settings: defaults(), avatar: null, background: null, warnings: [] },
        hasSession: false,
        canClearSession: false,
        authMethod: null,
        warning: null,
      }
    if (command === 'transfer_snapshot')
      return { records: [], errors: {}, speeds: {}, persistence_error: null }
    if (command === 'cancel_page' || command === 'cancel_preview') return undefined
    throw { code: 'no_session', message: '没有保存的账号' }
  })
  const wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  await wrapper.find('.local-history-links button').trigger('click')
  expect(wrapper.find('.content-panel h1').text()).toBe('最近访问')
  await wrapper
    .findAll('.sidebar nav button')
    .find((button) => button.text().trim() === '下载')!
    .trigger('click')
  await flushPromises()
  expect(wrapper.find('.content-panel h1').text()).toBe('下载')
  const groups = wrapper.findAll('.task-filter-group')
  expect(groups).toHaveLength(1)
  expect(groups[0].findAll('button.active')).toHaveLength(1)
  await groups[0]
    .findAll('button')
    .find((button) => button.text().includes('失败'))!
    .trigger('click')
  expect(wrapper.findAll('.task-filter-group')[0].findAll('button.active')).toHaveLength(1)
  expect(wrapper.text()).not.toContain('云端离线')
  expect(wrapper.text()).not.toContain('连接 PikPak 后查看云端任务')
  wrapper.unmount()
})
it('完成下载从本机任务记录打开文件或所在目录', async () => {
  transport.call.mockImplementation(async (command: string) => {
    if (command === 'bootstrap')
      return {
        appearance: { settings: defaults(), avatar: null, background: null, warnings: [] },
        hasSession: false,
        canClearSession: false,
        authMethod: null,
        warning: null,
      }
    if (command === 'transfer_snapshot')
      return {
        records: [
          {
            id: 'task-a',
            file_id: 'file-a',
            file_name: '完成.mp4',
            completed: 100,
            total: 100,
            phase: 'Completed',
            local_path: 'C:/Downloads/完成.mp4',
          },
        ],
        errors: {},
        speeds: {},
        persistence_error: null,
      }
    if (command === 'cancel_page' || command === 'cancel_preview') return undefined
    if (command === 'open_transfer_location') return undefined
    throw { code: 'no_session', message: '没有保存的账号' }
  })
  const wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  await wrapper.findAll('.local-history-links button')[1].trigger('click')
  await flushPromises()
  await wrapper.find('[aria-label="打开下载文件"]').trigger('click')
  await wrapper.find('[aria-label="打开所在文件夹"]').trigger('click')
  expect(transport.call).toHaveBeenCalledWith('open_transfer_location', {
    id: 'task-a',
    revealFile: true,
  })
  expect(transport.call).toHaveBeenCalledWith('open_transfer_location', {
    id: 'task-a',
    revealFile: false,
  })
  wrapper.unmount()
})
it('视频详情调用专用媒体链接解析器，关闭侧栏时取消预览', async () => {
  transport.call.mockImplementation(async (command: string) => {
    if (command === 'bootstrap')
      return {
        appearance: { settings: defaults(), avatar: null, background: null, warnings: [] },
        hasSession: true,
        canClearSession: true,
        authMethod: 'personal_access_token',
        warning: null,
      }
    if (command === 'transfer_snapshot')
      return { records: [], errors: {}, speeds: {}, persistence_error: null }
    if (command === 'list_files')
      return [
        {
          id: 'video-a',
          name: '转码视频.mp4',
          kind: 'drive#file',
          size: 1024,
          mime_type: 'video/mp4',
          modified_time: '',
        },
      ]
    if (command === 'preview_content_link') return 'https://media.example.test/720p.mp4'
    if (command === 'cancel_page' || command === 'cancel_preview') return undefined
  })
  const wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  await wrapper.find('.file-item').trigger('click')
  await flushPromises()
  expect(transport.call).toHaveBeenCalledWith('preview_content_link', {
    id: 'video-a',
    generation: expect.any(Number),
  })
  expect(wrapper.find('video').attributes('src')).toBe('https://media.example.test/720p.mp4')
  await wrapper.find('[aria-label="关闭文件详情"]').trigger('click')
  await flushPromises()
  expect(wrapper.find('video').exists()).toBe(false)
  expect(transport.call.mock.calls.some((args) => args[0] === 'cancel_preview')).toBe(true)
  wrapper.unmount()
})
it('不会收集或提交不受支持的用户名密码登录', async () => {
  const original = transport.call.getMockImplementation()!
  transport.call.mockImplementation(async (command: string, ...args: unknown[]) => {
    if (command === 'bootstrap')
      return {
        appearance: { settings: defaults(), avatar: null, background: null, warnings: [] },
        hasSession: false,
        canClearSession: false,
        authMethod: null,
        warning: null,
      }
    return original(command, ...args)
  })
  const wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  const passwordOption = wrapper
    .findAll('button')
    .find((button) => button.text().includes('用户名/邮箱与密码登录'))!
  expect(passwordOption.attributes('disabled')).toBeDefined()
  expect(wrapper.text()).toContain('官方 CLI 已说明密码登录不再支持')
  expect(wrapper.findAll('input[type="password"]')).toHaveLength(1)
  expect(wrapper.find('input[type="password"]').attributes('id')).toBe('personal-access-token')
  wrapper.unmount()
})
