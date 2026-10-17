<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Folder, File, Image, Film, Music, Check } from '@lucide/vue'
import { isFolder, formatBytes, formatDate, type CloudFile } from '../types'
const props = defineProps<{
  files: CloudFile[]
  grid: boolean
  selected: Set<string>
  selectionMode?: boolean
}>()
const emit = defineEmits<{
  select: [file: CloudFile, modifiers: { multi: boolean; range: boolean }]
  open: [file: CloudFile]
  menu: [file: CloudFile, event: MouseEvent | KeyboardEvent]
}>()
const viewport = ref<HTMLElement>()
const scroll = ref(0)
const height = ref(500)
const width = ref(700)
const columns = computed(() => (props.grid ? Math.max(1, Math.floor(width.value / 210)) : 1))
const rowHeight = computed(() => (props.grid ? 164 : 48))
const start = computed(() => Math.max(0, Math.floor(scroll.value / rowHeight.value) - 3))
const totalRows = computed(() => Math.ceil(props.files.length / columns.value))
const rows = computed(() =>
  Array.from(
    {
      length: Math.max(
        0,
        Math.min(totalRows.value, start.value + Math.ceil(height.value / rowHeight.value) + 6) -
          start.value,
      ),
    },
    (_, i) => {
      const index = start.value + i
      return { index, files: props.files.slice(index * columns.value, (index + 1) * columns.value) }
    },
  ),
)
watch(
  [() => props.files, () => props.grid],
  () => {
    scroll.value = 0
    if (viewport.value) viewport.value.scrollTop = 0
  },
  { flush: 'post' },
)
let observer: ResizeObserver | undefined
onMounted(() => {
  observer = new ResizeObserver((entries) => {
    const box = entries[0]?.contentRect
    if (box) {
      height.value = box.height
      width.value = box.width
    }
  })
  if (viewport.value) observer.observe(viewport.value)
})
onBeforeUnmount(() => observer?.disconnect())
const icon = (file: CloudFile) =>
  isFolder(file)
    ? Folder
    : file.mime_type.startsWith('image/')
      ? Image
      : file.mime_type.startsWith('video/')
        ? Film
        : file.mime_type.startsWith('audio/')
          ? Music
          : File
function activate(event: MouseEvent, file: CloudFile) {
  if (props.selectionMode || event.ctrlKey || event.metaKey || event.shiftKey) {
    emit('select', file, {
      multi: event.ctrlKey || event.metaKey || props.selectionMode || props.selected.has(file.id),
      range: event.shiftKey,
    })
  } else emit('open', file)
}
function keyboard(event: KeyboardEvent, file: CloudFile) {
  if (event.key === ' ' || event.key === 'Spacebar') {
    event.preventDefault()
    emit('select', file, {
      multi: event.ctrlKey || event.metaKey || props.selectionMode || props.selected.has(file.id),
      range: event.shiftKey,
    })
  } else if (event.key === 'Enter') {
    event.preventDefault()
    emit('open', file)
  } else if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
    event.preventDefault()
    emit('menu', file, event)
  }
}
</script>
<template>
  <div
    ref="viewport"
    class="file-viewport"
    :class="{ 'selection-mode': selectionMode }"
    role="region"
    aria-label="文件区域"
    @scroll="scroll = ($event.target as HTMLElement).scrollTop"
  >
    <div class="virtual-canvas" :style="{ height: `${totalRows * rowHeight}px` }">
      <div
        v-for="row in rows"
        :key="row.index"
        class="virtual-row"
        :class="{ grid }"
        :style="{
          height: `${rowHeight}px`,
          transform: `translateY(${row.index * rowHeight}px)`,
          gridTemplateColumns: `repeat(${columns},minmax(0,1fr))`,
        }"
      >
        <div
          v-for="file in row.files"
          :key="file.id"
          class="file-item"
          :class="{ selected: selected.has(file.id), folder: isFolder(file) }"
          role="group"
          :aria-label="file.name"
          :aria-selected="selected.has(file.id)"
          tabindex="0"
          @click="activate($event, file)"
          @keydown="keyboard($event, file)"
          @contextmenu.prevent="emit('menu', file, $event)"
        >
          <button
            class="selection-box"
            :aria-label="`${selected.has(file.id) ? '取消选择' : '选择'} ${file.name}`"
            :aria-pressed="selected.has(file.id)"
            @click.stop="emit('select', file, { multi: true, range: $event.shiftKey })"
          >
            <Check v-if="selected.has(file.id)" :size="12" />
          </button>
          <component
            :is="icon(file)"
            class="file-icon"
            :size="grid ? 34 : 23"
            :stroke-width="1.6"
          />
          <span class="file-name" :title="file.name">{{ file.name }}</span>
          <span class="file-size">{{ isFolder(file) ? '文件夹' : formatBytes(file.size) }}</span>
          <span v-if="!grid" class="file-date">{{ formatDate(file.modified_time) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
