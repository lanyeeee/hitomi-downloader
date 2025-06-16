import { defineStore } from 'pinia'
import { Comic, Config } from './bindings.ts'
import { ref } from 'vue'

export const useStore = defineStore('store', () => {
  const config = ref<Config>()
  const pickedComic = ref<Comic>()
  return { config, pickedComic }
})
