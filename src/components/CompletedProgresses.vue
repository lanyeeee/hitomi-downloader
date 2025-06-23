<script setup lang="ts">
import { ProgressData } from '../types.ts'
import { computed } from 'vue'
import { useStore } from '../store.ts'
import DownloadedComicCard from './DownloadedComicCard.vue'

const store = useStore()

defineProps<{
  search: (query: string, pageNum: number) => Promise<void>
}>()

const completedProgresses = computed<[number, ProgressData][]>(() =>
  Array.from(store.progresses.entries())
    .filter(([, { state }]) => state === 'Completed')
    .sort((a, b) => {
      return b[1].totalImgCount - a[1].totalImgCount
    }),
)
</script>

<template>
  <div class="h-full flex flex-col overflow-auto px-2 gap-2">
    <downloaded-comic-card
      v-for="[comicId, progressData] in completedProgresses"
      :key="comicId"
      :search="search"
      v-model:comic="progressData.comic" />
  </div>
</template>
