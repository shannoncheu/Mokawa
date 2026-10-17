import { mount } from '@vue/test-utils'
import { defineComponent, nextTick, ref, shallowRef } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { useFileView } from '../src/useFileView'
import type { CloudFile } from '../src/types'
it('合并搜索请求，忽略旧结果，并在清空或销毁时终止 Worker', async () => {
  const workers: FakeWorker[] = []
  class FakeWorker {
    onmessage: ((event: { data: unknown }) => void) | null = null
    onerror: (() => void) | null = null
    postMessage = vi.fn()
    terminate = vi.fn()
    constructor() {
      workers.push(this)
    }
  }
  vi.stubGlobal('Worker', FakeWorker)
  const files = shallowRef([
    { id: '1', name: '文件1', kind: 'drive#file' },
    { id: '2', name: '文件2', kind: 'drive#file' },
  ] as CloudFile[])
  const query = ref('')
  const descending = ref(false)
  let view!: ReturnType<typeof useFileView>
  const wrapper = mount(
    defineComponent({
      setup() {
        view = useFileView(files, query, descending, vi.fn())
        return () => null
      },
    }),
  )
  const worker = workers[0]!
  expect(worker.postMessage).toHaveBeenCalledTimes(1)
  for (let i = 0; i < 100; i++) {
    query.value = String(i)
    await nextTick()
  }
  expect(worker.postMessage).toHaveBeenCalledTimes(1)
  worker.onmessage!({ data: { version: 1, indices: new Uint32Array([0]) } })
  expect(view.visible.value).toEqual([])
  expect(worker.postMessage).toHaveBeenCalledTimes(2)
  const latest = worker.postMessage.mock.calls[1]![0]
  expect(latest.query).toBe('99')
  expect(latest.rows).toBeUndefined()
  worker.onmessage!({ data: { version: latest.version, indices: new Uint32Array([1]) } })
  expect(view.visible.value.map((file) => file.id)).toEqual(['2'])
  files.value = []
  await nextTick()
  expect(worker.terminate).toHaveBeenCalledTimes(1)
  expect(worker.onmessage).toBeNull()
  expect(view.visible.value).toEqual([])
  files.value = [{ id: '3', name: '文件3', kind: 'drive#file' }] as CloudFile[]
  await nextTick()
  expect(workers).toHaveLength(2)
  wrapper.unmount()
  expect(workers[1]!.terminate).toHaveBeenCalledTimes(1)
})
afterEach(() => vi.unstubAllGlobals())
