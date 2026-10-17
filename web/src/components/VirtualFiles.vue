<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { Folder, File, Image, Film, Music, Check } from '@lucide/vue'
import { isFolder, formatBytes, formatDate, type CloudFile } from '../types'
const props = defineProps<{ files: CloudFile[]; grid: boolean; selected: Set<string> }>()
const emit = defineEmits<{
  select: [file: CloudFile, multi: boolean]
  open: [file: CloudFile]
  menu: [file: CloudFile, event: MouseEvent]
}>()
const viewport = ref<HTMLElement>()
const scroll = ref(0)
const height = ref(500)
const width = ref(700)
const columns = computed(() => (props.grid ? Math.max(1, Math.floor(width.value / 210)) : 1))
const rowHeight = computed(() => (props.grid ? 164 : 54))
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
</script>
<template>
  <div
    ref="viewport"
    class="file-viewport"
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
          tabindex="0"
          @click="emit('select', file, $event.ctrlKey || $event.metaKey)"
          @keydown.space.prevent="emit('select', file, true)"
          @keydown.enter="emit('open', file)"
          @dblclick="emit('open', file)"
          @contextmenu.prevent="emit('menu', file, $event)"
        >
          <button
            class="selection-box"
            :aria-label="`${selected.has(file.id) ? '取消选择' : '选择'} ${file.name}`"
            :aria-pressed="selected.has(file.id)"
            @click.stop="emit('select', file, true)"
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
