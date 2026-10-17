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
  await wrapper.find('.file-item').trigger('click')
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
it('无效保存会话不展示文件主页，也不展示退出登录', async () => {
  const original = transport.call.getMockImplementation()!
  transport.call.mockImplementation(async (command: string, ...args: unknown[]) => {
    if (command === 'list_files')
      throw { code: 'authentication', message: '会话已过期，请重新登录。' }
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
    if (command === 'rename_file') throw { code: 'authentication', message: '会话已过期' }
    return original(command, ...args)
  })
  const wrapper = mount(App, { attachTo: document.body })
  await flushPromises()
  await wrapper.find('.file-item').trigger('click')
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
