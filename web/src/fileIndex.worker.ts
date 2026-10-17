/// <reference lib="webworker" />
export interface IndexRow {
  name: string
  folder: boolean
}
export interface IndexJob {
  version: number
  query: string
  descending: boolean
  rows?: IndexRow[]
}
let rows: IndexRow[] = []
let lower: string[] = []
let order: number[] = []
let folders = 0
const collator = new Intl.Collator('zh-CN', { numeric: true, sensitivity: 'base' })
self.onmessage = (event: MessageEvent<IndexJob>) => {
  const job = event.data
  const started = performance.now()
  if (job.rows) {
    rows = job.rows
    lower = rows.map((row) => row.name.toLocaleLowerCase())
    order = rows
      .map((_row, index) => index)
      .sort(
        (a, b) =>
          Number(rows[b]!.folder) - Number(rows[a]!.folder) ||
          collator.compare(rows[a]!.name, rows[b]!.name),
      )
    folders = order.findIndex((index) => !rows[index]!.folder)
    if (folders < 0) folders = order.length
  }
  const query = job.query.toLocaleLowerCase()
  const found: number[] = []
  const add = (index: number) => {
    if (lower[index]!.includes(query)) found.push(index)
  }
  if (job.descending) {
    for (let i = folders - 1; i >= 0; i--) add(order[i]!)
    for (let i = order.length - 1; i >= folders; i--) add(order[i]!)
  } else for (const index of order) add(index)
  const indices = Uint32Array.from(found)
  self.postMessage({ version: job.version, indices, elapsed: performance.now() - started }, [
    indices.buffer,
  ])
}
