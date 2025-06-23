<script setup lang="tsx">
import { ProgressData } from '../types.ts'
import { ref, watch, computed, nextTick } from 'vue'
import { SelectionArea, SelectionEvent } from '@viselect/vue'
import { commands, DownloadTaskState } from '../bindings.ts'
import { DropdownOption, NIcon } from 'naive-ui'
import { useStore } from '../store.ts'
import {
  PauseOutline,
  CheckmarkDoneOutline,
  TrashOutline,
  ChevronForwardOutline,
  CloudDownloadOutline,
  TimeOutline,
  AlertCircleOutline,
} from '@vicons/ionicons5'
import { useI18n } from '../utils.ts'

const { t } = useI18n()

const store = useStore()

const selectedIds = ref<Set<number>>(new Set())
const selectionAreaRef = ref<InstanceType<typeof SelectionArea>>()
const selectableRefs = ref<HTMLDivElement[]>([])
const { dropdownX, dropdownY, dropdownShowing, dropdownOptions, showDropdown } = useDropdown()

const uncompletedProgresses = computed<[number, ProgressData][]>(() =>
  Array.from(store.progresses.entries())
    .filter(([, { state }]) => state !== 'Completed' && state !== 'Cancelled')
    .sort((a, b) => b[1].totalImgCount - a[1].totalImgCount),
)

watch(uncompletedProgresses, () => {
  const uncompletedIds = new Set(uncompletedProgresses.value.map(([chapterId]) => chapterId))
  // only retain selected ids that are uncompleted
  selectedIds.value = new Set([...selectedIds.value].filter((comicId) => uncompletedIds.has(comicId)))
})

function extractIds(elements: Element[]): number[] {
  return elements
    .map((element) => element.getAttribute('data-key'))
    .filter(Boolean)
    .map(Number)
}

function updateSelectedIds({
  store: {
    changed: { added, removed },
  },
}: SelectionEvent) {
  extractIds(added).forEach((comicId) => selectedIds.value.add(comicId))
  extractIds(removed).forEach((comicId) => selectedIds.value.delete(comicId))
}

function unselectAll({ event, selection }: SelectionEvent) {
  if (!event?.ctrlKey && !event?.metaKey) {
    selection.clearSelection()
    selectedIds.value.clear()
  }
}

async function onProgressDoubleClick(state: DownloadTaskState, comicId: number) {
  if (state === 'Downloading' || state === 'Pending') {
    const result = await commands.pauseDownloadTask(comicId)
    if (result.status === 'error') {
      console.error(result.error)
    }
  } else {
    const result = await commands.resumeDownloadTask(comicId)
    if (result.status === 'error') {
      console.error(result.error)
    }
  }
}

function onProgressContextMenu(comicId: number) {
  if (selectedIds.value.has(comicId)) {
    return
  }
  selectedIds.value.clear()
  selectedIds.value.add(comicId)
}

function useDropdown() {
  const dropdownX = ref<number>(0)
  const dropdownY = ref<number>(0)
  const dropdownShowing = ref<boolean>(false)
  const dropdownOptions: DropdownOption[] = [
    {
      label: t('common.select_all'),
      key: 'select-all',
      icon: () => (
        <n-icon>
          <CheckmarkDoneOutline />
        </n-icon>
      ),
      props: {
        onClick: () => {
          if (selectionAreaRef.value === undefined) {
            return
          }
          const selection = selectionAreaRef.value.selection
          if (selection === undefined) {
            return
          }
          selection.select(selectableRefs.value)
          dropdownShowing.value = false
        },
      },
    },
    {
      label: t('common.continue'),
      key: 'resume',
      icon: () => (
        <n-icon>
          <ChevronForwardOutline />
        </n-icon>
      ),
      props: {
        onClick: () => {
          selectedIds.value.forEach(async (comicId) => {
            const result = await commands.resumeDownloadTask(comicId)
            if (result.status === 'error') {
              console.error(result.error)
            }
          })
          dropdownShowing.value = false
        },
      },
    },
    {
      label: t('common.pause'),
      key: 'pause',
      icon: () => (
        <n-icon>
          <PauseOutline />
        </n-icon>
      ),
      props: {
        onClick: () => {
          selectedIds.value.forEach(async (comicId) => {
            const result = await commands.pauseDownloadTask(comicId)
            if (result.status === 'error') {
              console.error(result.error)
            }
          })
          dropdownShowing.value = false
        },
      },
    },
    {
      label: t('common.cancel'),
      key: 'cancel',
      icon: () => (
        <n-icon>
          <TrashOutline />
        </n-icon>
      ),
      props: {
        onClick: () => {
          selectedIds.value.forEach(async (comicId) => {
            const result = await commands.cancelDownloadTask(comicId)
            if (result.status === 'error') {
              console.error(result.error)
            }
          })
          dropdownShowing.value = false
        },
      },
    },
  ]

  async function showDropdown(e: MouseEvent) {
    dropdownShowing.value = false
    await nextTick()
    dropdownShowing.value = true
    dropdownX.value = e.clientX
    dropdownY.value = e.clientY
  }

  return {
    dropdownX,
    dropdownY,
    dropdownShowing,
    dropdownOptions,
    showDropdown,
  }
}

function stateToStatus(state: DownloadTaskState): 'default' | 'info' | 'success' | 'warning' | 'error' {
  if (state === 'Completed') {
    return 'success'
  } else if (state === 'Paused') {
    return 'warning'
  } else if (state === 'Failed') {
    return 'error'
  } else {
    return 'default'
  }
}

function stateToColorClass(state: DownloadTaskState) {
  if (state === 'Downloading') {
    return 'text-blue-500'
  } else if (state === 'Pending') {
    return 'text-gray-500'
  } else if (state === 'Paused') {
    return 'text-yellow-500'
  } else if (state === 'Failed') {
    return 'text-red-500'
  } else if (state === 'Completed') {
    return 'text-green-500'
  } else if (state === 'Cancelled') {
    return 'text-stone-500'
  }

  return ''
}
</script>

<template>
  <SelectionArea
    ref="selectionAreaRef"
    class="h-full flex flex-col selection-container box-border px-2"
    :options="{ selectables: '.selectable', features: { deselectOnBlur: true } }"
    @contextmenu="showDropdown"
    @move="updateSelectedIds"
    @start="unselectAll">
    <span class="ml-auto select-none">{{ t('uncompleted_progresses.usage_tips') }}</span>
    <div class="h-full select-none">
      <div
        v-for="[comicId, { state, comic, percentage, indicator }] in uncompletedProgresses"
        :key="comicId"
        ref="selectableRefs"
        :data-key="comicId"
        :class="[
          'selectable p-3 m-1 mb-2 rounded-lg',
          selectedIds.has(comicId) ? 'selected shadow-md' : 'hover:bg-gray-1',
        ]"
        @dblclick="() => onProgressDoubleClick(state, comicId)"
        @contextmenu="() => onProgressContextMenu(comicId)">
        <div class="flex items-center" :title="comic.title">
          <div class="text-ellipsis whitespace-nowrap overflow-hidden">{{ comic.title }}</div>
        </div>
        <div class="flex items-center" :title="comic.title">
          <n-icon :class="[stateToColorClass(state), 'mr-2']" :size="20">
            <CloudDownloadOutline v-if="state === 'Downloading'" />
            <TimeOutline v-else-if="state === 'Pending'" />
            <PauseOutline v-else-if="state === 'Paused'" />
            <AlertCircleOutline v-else-if="state === 'Failed'" />
          </n-icon>
          <n-progress
            :class="stateToColorClass(state)"
            :status="stateToStatus(state)"
            :percentage="percentage"
            :processing="state === 'Downloading'">
            {{ indicator }}
          </n-progress>
        </div>
      </div>
    </div>
    <n-dropdown
      placement="bottom-start"
      trigger="manual"
      :x="dropdownX"
      :y="dropdownY"
      :options="dropdownOptions"
      :show="dropdownShowing"
      :on-clickoutside="() => (dropdownShowing = false)" />
  </SelectionArea>
</template>

<style scoped>
.selection-container {
  @apply select-none overflow-auto;
}

.selection-container .selected {
  @apply bg-[rgb(204,232,255)];
}

:global(.selection-area) {
  @apply bg-[rgba(46,115,252,0.5)];
}
</style>
