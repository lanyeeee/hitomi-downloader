<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { events, commands } from '../bindings.ts'
import { open } from '@tauri-apps/plugin-dialog'
import UncompletedProgresses from '../components/UncompletedProgresses.vue'
import CompletedProgresses from '../components/CompletedProgresses.vue'
import { useStore } from '../store.ts'
import { useI18n } from '../utils.ts'
import { PhFolderOpen, PhGearSix } from '@phosphor-icons/vue'
import SettingsDialog from '../components/SettingsDialog.vue'
import { ProgressData } from '../types.ts'

const { t } = useI18n()

const store = useStore()

defineProps<{
  search: (query: string, pageNum: number) => Promise<void>
}>()

const downloadSpeed = ref<string>('')
const settingsDialogShowing = ref<boolean>(false)

onMounted(async () => {
  await events.downloadSpeedEvent.listen(async ({ payload: { speed } }) => {
    downloadSpeed.value = speed
  })

  await events.downloadTaskEvent.listen(async ({ payload: { event, data } }) => {
    if (event === 'Create') {
      const { comic } = data

      store.progresses.set(comic.id, {
        ...data,
        percentage: 0,
        indicator: t('downloading_pane.pending'),
      })
    } else if (event === 'Update') {
      const { comicId, state, downloadedImgCount, totalImgCount } = data

      const progressData = store.progresses.get(comicId)
      if (progressData === undefined) {
        return
      }

      progressData.state = state
      progressData.downloadedImgCount = downloadedImgCount
      progressData.totalImgCount = totalImgCount

      if (state === 'Completed') {
        progressData.comic.isDownloaded = true

        await syncPickedComic()
        await syncComicInSearch(progressData)
      }

      progressData.percentage = (downloadedImgCount / totalImgCount) * 100

      let indicator = ''
      if (state === 'Pending') {
        indicator = t('downloading_pane.pending')
      } else if (state === 'Downloading') {
        indicator = t('downloading_pane.downloading')
      } else if (state === 'Paused') {
        indicator = t('downloading_pane.paused')
      } else if (state === 'Cancelled') {
        indicator = t('downloading_pane.cancelled')
      } else if (state === 'Completed') {
        indicator = t('downloading_pane.completed')
      } else if (state === 'Failed') {
        indicator = t('downloading_pane.failed')
      }
      if (totalImgCount !== 0) {
        indicator += ` ${downloadedImgCount}/${totalImgCount}`
      }

      progressData.indicator = indicator
    }
  })
})

async function syncPickedComic() {
  if (store.pickedComic === undefined) {
    return
  }
  const result = await commands.getSyncedComic(store.pickedComic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  store.pickedComic = result.data
}

async function syncComicInSearch(progressData: ProgressData) {
  if (store.searchResult === undefined) {
    return
  }
  const comic = store.searchResult.comics.find((comic) => comic.id === progressData.comic.id)
  if (comic === undefined) {
    return
  }
  const result = await commands.getSyncedComic(comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  Object.assign(comic, { ...result.data })
}

// Select download directory through dialog
async function selectDownloadDir() {
  if (store.config === undefined) {
    return
  }

  const selectedDirPath = await open({ directory: true })
  if (selectedDirPath === null) {
    return
  }
  store.config.downloadDir = selectedDirPath
}

async function showDownloadDirInFileManager() {
  if (store.config === undefined) {
    return
  }

  const result = await commands.showPathInFileManager(store.config.downloadDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <div v-if="store.config !== undefined" class="flex flex-col gap-2">
    <div class="flex gap-1 box-border px-2 pt-2">
      <n-input-group>
        <n-input-group-label size="small">
          {{ t('common.download_directory') }}
        </n-input-group-label>
        <n-input v-model:value="store.config.downloadDir" size="small" readonly @click="selectDownloadDir" />
        <n-button class="w-9" size="small" @click="showDownloadDirInFileManager">
          <template #icon>
            <n-icon size="20">
              <PhFolderOpen />
            </n-icon>
          </template>
        </n-button>
      </n-input-group>
      <n-button @click="settingsDialogShowing = true" size="small">
        <template #icon>
          <n-icon size="20">
            <PhGearSix />
          </n-icon>
        </template>
        {{ t('settings_dialog.name') }}
      </n-button>
    </div>

    <n-tabs class="h-full overflow-auto" type="line" size="small" animated>
      <n-tab-pane class="h-full p-0! overflow-auto" name="uncompleted" :tab="t('uncompleted_progresses.name')">
        <uncompleted-progresses />
      </n-tab-pane>
      <n-tab-pane class="h-full p-0! overflow-auto" name="completed" :tab="t('completed_progresses.name')">
        <completed-progresses :search="search" />
      </n-tab-pane>
    </n-tabs>

    <span class="ml-auto mr-2 mb-2">{{ t('downloading_pane.download_speed') }}: {{ downloadSpeed }}</span>

    <settings-dialog v-model:showing="settingsDialogShowing" />
  </div>
</template>

<style scoped>
:deep(.n-progress-content) {
  @apply h-full;
}

:deep(.n-tabs-tab) {
  @apply pt-0;
}
</style>
