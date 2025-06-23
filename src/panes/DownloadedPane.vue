<script setup lang="ts">
import { Comic, commands, events } from '../bindings.ts'
import { computed, onMounted, ref, watch } from 'vue'
import { MessageReactive, useMessage } from 'naive-ui'
import { open } from '@tauri-apps/plugin-dialog'
import { useStore } from '../store.ts'
import DownloadedComicCard from '../components/DownloadedComicCard.vue'
import { useI18n } from '../utils.ts'
import { FolderOpenOutline } from '@vicons/ionicons5'

const { t } = useI18n()

interface ProgressData {
  title: string
  progressMessage: MessageReactive
}

defineProps<{
  search: (query: string, pageNum: number) => Promise<void>
}>()

const store = useStore()

const message = useMessage()

const comicCardContainerRef = ref<HTMLElement>()

const PAGE_SIZE = 20

const downloadedComics = ref<Comic[]>([])
const currentPage = ref<number>(1)
const pageCount = computed<number>(() => {
  return Math.ceil(downloadedComics.value.length / PAGE_SIZE)
})
// currentPageComics is a computed property that returns the comics on the current page
const currentPageComics = computed<Comic[]>(() => {
  const start = (currentPage.value - 1) * PAGE_SIZE
  const end = start + PAGE_SIZE
  return downloadedComics.value.slice(start, end)
})

watch(currentPage, () => {
  if (comicCardContainerRef.value !== undefined) {
    comicCardContainerRef.value.scrollTo({ top: 0, behavior: 'instant' })
  }
})

// listen for changes in the tab and update the list of downloaded comics
watch(
  () => store.currentTabName,
  async () => {
    if (store.currentTabName !== 'downloaded') {
      return
    }

    const result = await commands.getDownloadedComics()
    if (result.status === 'error') {
      console.error(result.error)
      return
    }

    downloadedComics.value = result.data
  },
  { immediate: true },
)

useProgressTracking()

function useProgressTracking() {
  const progresses = new Map<string, ProgressData>()

  async function handleExportPdfEvents() {
    await events.exportPdfEvent.listen(async ({ payload: exportEvent }) => {
      if (exportEvent.event === 'Start') {
        const { uuid, title } = exportEvent.data
        createProgress(uuid, title, t('downloaded_pane.pdf_exporting'))
      } else if (exportEvent.event === 'Error') {
        errorProgress(exportEvent.data.uuid, t('downloaded_pane.pdf_export_error'))
      } else if (exportEvent.event === 'End') {
        completeProgress(exportEvent.data.uuid, t('downloaded_pane.pdf_exported'))
      }
    })
  }

  async function handleExportCbzEvents() {
    await events.exportCbzEvent.listen(async ({ payload: exportEvent }) => {
      if (exportEvent.event === 'Start') {
        const { uuid, title } = exportEvent.data
        createProgress(uuid, title, t('downloaded_pane.cbz_exporting'))
      } else if (exportEvent.event === 'Error') {
        errorProgress(exportEvent.data.uuid, t('downloaded_pane.cbz_export_error'))
      } else if (exportEvent.event === 'End') {
        completeProgress(exportEvent.data.uuid, t('downloaded_pane.cbz_exported'))
      }
    })
  }

  // Create progress message
  function createProgress(uuid: string, title: string, actionMessage: string) {
    progresses.set(uuid, {
      title,
      progressMessage: message.loading(
        () => {
          const progressData = progresses.get(uuid)
          if (progressData === undefined) return ''
          return `${progressData.title} ${actionMessage}`
        },
        { duration: 0 },
      ),
    })
  }

  function errorProgress(uuid: string, actionMessage: string) {
    const progressData = progresses.get(uuid)
    if (progressData) {
      progressData.progressMessage.type = 'error'
      progressData.progressMessage.content = `${progressData.title} ${actionMessage}`
      setTimeout(() => {
        progressData.progressMessage.destroy()
        progresses.delete(uuid)
      }, 3000)
    }
  }

  // Set the progress message as complete
  function completeProgress(uuid: string, actionMessage: string) {
    const progressData = progresses.get(uuid)
    if (progressData) {
      progressData.progressMessage.type = 'success'
      progressData.progressMessage.content = `${progressData.title} ${actionMessage}`
      setTimeout(() => {
        progressData.progressMessage.destroy()
        progresses.delete(uuid)
      }, 3000)
    }
  }

  onMounted(async () => {
    await handleExportPdfEvents()
    await handleExportCbzEvents()
  })
}

async function selectExportDir() {
  if (store.config === undefined) {
    return
  }

  const selectedDirPath = await open({ directory: true })
  if (selectedDirPath === null) {
    return
  }
  store.config.exportDir = selectedDirPath
}

async function showExportDirInFileManager() {
  if (store.config === undefined) {
    return
  }
  console.log(currentPageComics.value)

  const result = await commands.showPathInFileManager(store.config.exportDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <div v-if="store.config !== undefined" class="h-full flex flex-col gap-2">
    <n-input-group class="box-border px-2 pt-2">
      <n-input-group-label size="small">
        {{ t('common.export_directory') }}
      </n-input-group-label>
      <n-input v-model:value="store.config.exportDir" size="small" readonly @click="selectExportDir" />
      <n-button class="w-9" size="small" @click="showExportDirInFileManager">
        <template #icon>
          <n-icon>
            <FolderOpenOutline />
          </n-icon>
        </template>
      </n-button>
    </n-input-group>

    <div ref="comicCardContainerRef" class="flex flex-col gap-row-2 overflow-auto box-border px-2">
      <downloaded-comic-card v-for="comic in currentPageComics" :key="comic.id" :search="search" :comic="comic" />
    </div>

    <n-pagination
      class="box-border p-2 pt-0 mt-auto"
      :page-count="pageCount"
      :page="currentPage"
      @update:page="currentPage = $event" />
  </div>
</template>
