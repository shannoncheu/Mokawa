import { computed, onBeforeUnmount, ref, shallowRef, watch, type ShallowRef, type Ref } from 'vue'
import { isFolder, type CloudFile } from './types'
import type { IndexJob } from './fileIndex.worker'
export function useFileView(
  files: ShallowRef<CloudFile[]>,
  query: Ref<string>,
  descending: Ref<boolean>,
  notice: (message: string) => void,
) {
  const indices = shallowRef<Uint32Array>(new Uint32Array())
  const indexing = ref(false)
  let worker: Worker | undefined,
    busy = false,
    version = 0,
    dataRevision = 0,
    postedRevision = -1,
    lastFiles: CloudFile[] | undefined,
    pending: (IndexJob & { revision: number }) | undefined
  function stop() {
    if (worker) {
      worker.onmessage = null
      worker.onerror = null
      worker.terminate()
    }
    worker = undefined
    pending = undefined
    busy = false
    postedRevision = -1
    indices.value = new Uint32Array()
    indexing.value = false
  }
  function post() {
    if (!pending || busy || !worker) return
    const job = pending
    pending = undefined
    busy = true
    if (job.revision !== postedRevision) {
      job.rows = files.value.map((file) => ({ name: file.name, folder: isFolder(file) }))
      postedRevision = job.revision
    }
    worker.postMessage(job)
  }
  watch(
    [files, query, descending],
    () => {
      version++
      if (files.value !== lastFiles) {
        dataRevision++
        lastFiles = files.value
      }
      if (!files.value.length) {
        stop()
        return
      }
      if (!worker) {
        try {
          worker = new Worker(new URL('./fileIndex.worker.ts', import.meta.url), { type: 'module' })
          worker.onmessage = (event: MessageEvent<{ version: number; indices: Uint32Array }>) => {
            busy = false
            if (event.data.version === version) {
              indices.value = event.data.indices
              indexing.value = false
            }
            post()
          }
          worker.onerror = () => {
            stop()
            notice('文件列表准备失败，请刷新重试。')
          }
        } catch {
          stop()
          notice('无法启动文件列表处理，请重新打开应用。')
          return
        }
      }
      indexing.value = true
      pending = {
        version,
        revision: dataRevision,
        query: query.value,
        descending: descending.value,
      }
      post()
    },
    { immediate: true },
  )
  onBeforeUnmount(stop)
  const visible = computed(() =>
    Array.from(indices.value, (index) => files.value[index]!).filter(Boolean),
  )
  return { visible, indexing }
}
