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
  <div class="h-full min-h-0 flex flex-col gap-2 overflow-auto px-2 pb-2">
    <downloaded-comic-card
      v-for="[comicId, progressData] in completedProgresses"
      :key="comicId"
      :search="search"
      :comic="progressData.comic" />
  </div>
</template>
