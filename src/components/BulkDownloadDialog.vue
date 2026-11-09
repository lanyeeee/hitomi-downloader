<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useNotification } from 'naive-ui'
import { Comic, DownloadTaskState, commands, events } from '../bindings.ts'
import { useStore } from '../store.ts'
import { useI18n } from '../utils.ts'

type PageOp = '>' | '<' | '=='

const props = defineProps<{
  ids: number[]
  totalPage: number
  cachedPage: number
  cachedComics: Comic[]
  pageFilter: { op: PageOp; value: number } | null
}>()

const showing = defineModel<boolean>('showing', { required: true })

const { t } = useI18n()

const store = useStore()
const notification = useNotification()

const collecting = ref<boolean>(false)
const collectionCancelled = ref<boolean>(false)
const collectionPage = ref<number>(0)
const collectionError = ref<boolean>(false)
const comics = ref<Comic[]>([])

const textFilter = ref<string>('')
const selectedIds = ref<Set<number>>(new Set())

const downloading = ref<boolean>(false)
const queueCancelled = ref<boolean>(false)
const downloadCurrent = ref<number>(0)
const downloadTotal = ref<number>(0)
const downloadTitle = ref<string>('')
const completedCount = ref<number>(0)
const failedCount = ref<number>(0)
const finished = ref<boolean>(false)

const terminalStates: DownloadTaskState[] = ['Completed', 'Failed', 'Cancelled', 'Paused']

const filteredComics = computed<Comic[]>(() => {
  const needle = textFilter.value.trim().toLowerCase()
  if (needle === '') {
    return comics.value
  }
  return comics.value.filter((comic) => comicMatches(comic, needle))
})

const visibleSelectedCount = computed<number>(
  () => filteredComics.value.filter((comic) => selectedIds.value.has(comic.id)).length,
)
const allVisibleSelected = computed<boolean>(
  () => filteredComics.value.length > 0 && visibleSelectedCount.value === filteredComics.value.length,
)
const someVisibleSelected = computed<boolean>(
  () => visibleSelectedCount.value > 0 && !allVisibleSelected.value,
)

const collectionStatus = computed<string>(() =>
  t('bulk_download.collecting_page', { current: collectionPage.value, total: props.totalPage }),
)

const downloadStatus = computed<string>(() =>
  t('bulk_download.downloading_item', {
    current: downloadCurrent.value,
    total: downloadTotal.value,
    title: downloadTitle.value,
  }),
)

const summary = computed<string>(() =>
  t('bulk_download.finished', { completed: completedCount.value, failed: failedCount.value }),
)

function plainText(html: string): string {
  return html.replace(/<[^>]*>/g, '')
}

function comicMatches(comic: Comic, needle: string): boolean {
  const haystack = [
    plainText(comic.title),
    comic.type,
    comic.language,
    comic.languageLocalname,
    ...comic.artists,
    ...comic.parodys,
    ...comic.tags.map(({ tag }) => tag),
  ]
    .join(' ')
    .toLowerCase()
  return haystack.includes(needle)
}

function applyPageFilter(list: Comic[]): Comic[] {
  const filter = props.pageFilter
  if (filter === null) {
    return list
  }
  return list.filter((comic) => {
    const pages = comic.files?.length ?? 0
    if (filter.op === '>') return pages > filter.value
    if (filter.op === '<') return pages < filter.value
    return pages === filter.value
  })
}

function resetState() {
  collectionCancelled.value = false
  collectionError.value = false
  collectionPage.value = 0
  comics.value = []
  textFilter.value = ''
  selectedIds.value = new Set()
  queueCancelled.value = false
  downloading.value = false
  downloadCurrent.value = 0
  downloadTotal.value = 0
  downloadTitle.value = ''
  completedCount.value = 0
  failedCount.value = 0
  finished.value = false
}

async function startCollection() {
  resetState()
  collecting.value = true

  const collected: Comic[] = []
  for (let page = 1; page <= props.totalPage; page++) {
    if (collectionCancelled.value) {
      collecting.value = false
      return
    }
    collectionPage.value = page

    if (page === props.cachedPage) {
      collected.push(...props.cachedComics)
      continue
    }

    const result = await commands.getPage(props.ids, page)
    if (collectionCancelled.value) {
      collecting.value = false
      return
    }
    if (result.status === 'error') {
      console.error(result.error)
      collectionError.value = true
      collecting.value = false
      notification.error({ title: () => t('bulk_download.collect_failed'), duration: 4000 })
      return
    }
    collected.push(...result.data.comics)
  }

  comics.value = applyPageFilter(collected)
  selectedIds.value = new Set(comics.value.map(({ id }) => id))
  collecting.value = false
}

function toggleAll(checked: boolean) {
  const next = new Set(selectedIds.value)
  for (const { id } of filteredComics.value) {
    if (checked) {
      next.add(id)
    } else {
      next.delete(id)
    }
  }
  selectedIds.value = next
}

function toggleComic(id: number, checked: boolean) {
  const next = new Set(selectedIds.value)
  if (checked) {
    next.add(id)
  } else {
    next.delete(id)
  }
  selectedIds.value = next
}

async function downloadAndWait(comic: Comic): Promise<DownloadTaskState | 'Error' | 'CancelledByUser'> {
  const detail = await commands.getComic(comic.id)
  if (detail.status === 'error') {
    return 'Error'
  }

  const id = comic.id
  const previousState = store.progresses.get(id)?.state
  const alreadyActive =
    previousState === 'Pending' || previousState === 'Downloading' || previousState === 'Paused'

  let started = alreadyActive
  let settled = false
  let resolveWait: (state: DownloadTaskState | 'Error' | 'CancelledByUser') => void = () => {}
  const completion = new Promise<DownloadTaskState | 'Error' | 'CancelledByUser'>((resolve) => {
    resolveWait = resolve
  })

  const unlisten = await events.downloadTaskEvent.listen(({ payload }) => {
    if (settled) {
      return
    }
    if (payload.event === 'Create' && payload.data.comic.id === id) {
      started = true
      return
    }
    if (payload.event === 'Update' && payload.data.comicId === id && started) {
      if (terminalStates.includes(payload.data.state)) {
        settled = true
        resolveWait(payload.data.state)
      }
    }
  })

  const cancelInterval = window.setInterval(() => {
    if (!settled && queueCancelled.value) {
      settled = true
      resolveWait('CancelledByUser')
    }
  }, 200)

  const created =
    previousState === 'Paused'
      ? await commands.resumeDownloadTask(id)
      : await commands.createDownloadTask(detail.data)
  if (created.status === 'error') {
    settled = true
    resolveWait('Error')
  }

  const state = await completion
  settled = true
  window.clearInterval(cancelInterval)
  await unlisten()
  return state
}

async function startDownload() {
  if (downloading.value || selectedIds.value.size === 0) {
    return
  }

  downloading.value = true
  finished.value = false
  queueCancelled.value = false
  completedCount.value = 0
  failedCount.value = 0

  const targets = comics.value.filter(({ id }) => selectedIds.value.has(id))
  downloadTotal.value = targets.length

  for (const [index, comic] of targets.entries()) {
    if (queueCancelled.value) {
      break
    }
    downloadCurrent.value = index + 1
    downloadTitle.value = plainText(comic.title)

    const state = await downloadAndWait(comic)
    if (state === 'Completed') {
      completedCount.value++
    } else if (state === 'Failed' || state === 'Error') {
      failedCount.value++
    } else if (state === 'Paused' || state === 'Cancelled' || state === 'CancelledByUser') {
      break
    }
  }

  downloading.value = false
  finished.value = true
  if (queueCancelled.value) {
    notification.warning({ title: () => t('bulk_download.queue_cancelled'), duration: 4000 })
  } else {
    notification.success({ title: () => summary.value, duration: 4000 })
  }
}

function cancelCollection() {
  collectionCancelled.value = true
  showing.value = false
}

function cancelQueue() {
  queueCancelled.value = true
}

watch(textFilter, () => {
  // Keep the selection in sync with what the text filter shows
  const visible = new Set(filteredComics.value.map(({ id }) => id))
  selectedIds.value = new Set([...selectedIds.value].filter((id) => visible.has(id)))
})

watch(
  showing,
  (value) => {
    if (value) {
      void startCollection()
    } else {
      collectionCancelled.value = true
      queueCancelled.value = true
    }
  },
  { immediate: true },
)
</script>

<template>
  <n-modal
    v-model:show="showing"
    preset="card"
    style="width: min(900px, 95vw)"
    :title="t('bulk_download.name')"
    :mask-closable="false"
    :close-on-esc="!collecting && !downloading"
    :closable="!collecting && !downloading">
    <div class="flex flex-col gap-3">
      <div v-if="collecting" class="flex flex-col items-center gap-2 py-8">
        <n-spin size="medium" />
        <span class="text-sm opacity-70">{{ collectionStatus }}</span>
        <n-button size="small" @click="cancelCollection">{{ t('common.cancel') }}</n-button>
      </div>

      <div v-else-if="collectionError" class="flex flex-col items-center gap-2 py-8">
        <span class="text-sm">{{ t('bulk_download.collect_failed') }}</span>
        <n-button size="small" @click="showing = false">{{ t('bulk_download.close') }}</n-button>
      </div>

      <template v-else>
        <div class="flex flex-col gap-2">
          <n-input
            v-model:value="textFilter"
            size="small"
            clearable
            :disabled="downloading"
            :placeholder="t('bulk_download.filter_placeholder')" />
          <div class="flex items-center justify-between gap-2">
            <n-checkbox
              :checked="allVisibleSelected"
              :indeterminate="someVisibleSelected"
              :disabled="filteredComics.length === 0"
              @update:checked="toggleAll">
              {{ t('common.select_all') }}
            </n-checkbox>
            <span class="text-xs opacity-60">
              {{ t('bulk_download.selected_count', { selected: selectedIds.size, total: comics.length }) }}
            </span>
          </div>
        </div>

        <div
          class="max-h-[50vh] overflow-auto rounded border border-gray-200 dark:border-[#3b3f4d] divide-y divide-gray-100 dark:divide-[#3b3f4d]">
          <div v-if="filteredComics.length === 0" class="p-4 text-center text-sm opacity-60">
            {{ comics.length === 0 ? t('bulk_download.empty') : t('bulk_download.no_match') }}
          </div>
          <div
            v-for="comic in filteredComics"
            :key="comic.id"
            class="flex items-start gap-2 px-3 py-2 hover:bg-gray-50 dark:hover:bg-[#343b4f]">
            <n-checkbox
              :checked="selectedIds.has(comic.id)"
              :disabled="downloading"
              @update:checked="(checked: boolean) => toggleComic(comic.id, checked)" />
            <div class="flex min-w-0 flex-1 flex-col">
              <span class="truncate font-medium" :title="plainText(comic.title)">{{ plainText(comic.title) }}</span>
              <span class="truncate text-xs opacity-60">
                {{ comic.files.length }} {{ t('search_pane.pages') }}
                <template v-if="comic.artists.length"> · {{ comic.artists.join(', ') }}</template>
              </span>
            </div>
          </div>
        </div>

        <div v-if="downloading" class="flex items-center gap-2 text-sm">
          <n-spin size="small" />
          <span class="truncate">{{ downloadStatus }}</span>
        </div>
        <div v-else-if="finished" class="text-sm">{{ summary }}</div>

        <div class="flex justify-end gap-2">
          <template v-if="downloading">
            <n-button size="small" @click="cancelQueue">{{ t('bulk_download.cancel_queue') }}</n-button>
          </template>
          <template v-else>
            <n-button size="small" @click="showing = false">{{ t('bulk_download.close') }}</n-button>
            <n-button
              type="primary"
              size="small"
              :disabled="selectedIds.size === 0"
              @click="startDownload">
              {{ t('bulk_download.download_button', { count: selectedIds.size }) }}
            </n-button>
          </template>
        </div>
      </template>
    </div>
  </n-modal>
</template>
