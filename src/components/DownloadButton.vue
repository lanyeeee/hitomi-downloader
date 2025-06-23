<script setup lang="ts">
import { ProgressData } from '../types.ts'
import { computed } from 'vue'
import { commands } from '../bindings.ts'
import { useStore } from '../store.ts'
import { useI18n } from '../utils.ts'

const { t } = useI18n()

const props = withDefaults(
  defineProps<{
    type?: 'default' | 'tertiary' | 'primary' | 'success' | 'info' | 'warning' | 'error'
    size?: 'tiny' | 'small' | 'medium' | 'large'
    comicId: number
    comicDownloaded: boolean
  }>(),
  {
    type: 'default',
    size: 'medium',
  },
)

const store = useStore()

const comicProgress = computed<ProgressData | undefined>(() => {
  return store.progresses.get(props.comicId)
})

const buttonDisabled = computed<boolean>(() => {
  const state = comicProgress.value?.state
  return state === 'Downloading' || state === 'Pending'
})

const buttonIndicator = computed<string>(() => {
  if (comicProgress.value === undefined) {
    return props.comicDownloaded ? t('download_button.download_again') : t('download_button.quick_download')
  }

  const state = comicProgress.value.state

  if (state === 'Downloading' || state === 'Pending') {
    return comicProgress.value.indicator
  } else if (state === 'Paused') {
    return t('download_button.resume_download')
  } else {
    return t('download_button.download_again')
  }
})

async function onButtonClick() {
  const state = comicProgress.value?.state
  if (state === 'Downloading' || state === 'Pending') {
    return
  } else if (state === 'Paused') {
    const result = await commands.resumeDownloadTask(props.comicId)
    if (result.status === 'error') {
      console.error(result.error)
    }
  } else {
    const result = await commands.getComic(props.comicId)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }
    const comic = result.data
    await commands.createDownloadTask(comic)
  }
}
</script>

<template>
  <n-button :type="type" :size="size" @click="onButtonClick" :disabled="buttonDisabled">
    {{ buttonIndicator }}
  </n-button>
</template>

<style scoped></style>
