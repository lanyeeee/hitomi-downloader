import { defineStore } from 'pinia'
import { Comic, commands, Config, SearchResult } from './bindings.ts'
import { ref } from 'vue'
import { CurrentTabName } from './types.ts'

export const useStore = defineStore('store', () => {
  const config = ref<Config>()
  const pickedComic = ref<Comic>()
  const currentTabName = ref<CurrentTabName>('search')
  const covers = ref<Map<number, string>>(new Map())
  const searchResult = ref<SearchResult>()

  async function loadCover(id: number, url: string) {
    const result = await commands.getCoverData(url)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }
    const coverData: number[] = result.data
    const coverBlob = new Blob([new Uint8Array(coverData)])
    const cover = URL.createObjectURL(coverBlob)
    covers.value.set(id, cover)
  }

  return { config, pickedComic, currentTabName, covers, loadCover, searchResult }
})
