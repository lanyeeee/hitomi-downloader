import { defineStore } from 'pinia'
import { Comic, commands, Config, SearchResult } from './bindings.ts'
import { ref } from 'vue'
import { CurrentTabName, ProgressData } from './types.ts'

export type Theme = 'light' | 'dark' | 'system'

export const useStore = defineStore('store', () => {
  const config = ref<Config>({
    downloadDir: '',
    exportDir: '',
    enableFileLogger: false,
    downloadFormat: 'Webp',
    dirFmt: '',
    proxyHost: '',
    proxyMode: 'System',
    proxyPort: 0
  })
  const pickedComic = ref<Comic>()
  const currentTabName = ref<CurrentTabName>('search')
  const progresses = ref<Map<number, ProgressData>>(new Map())
  const covers = ref<Map<number, string>>(new Map())
  const searchResult = ref<SearchResult>()
  const theme = ref<Theme>('system')

  function setTheme(value: Theme) {
    theme.value = value
    localStorage.setItem('theme', value)
  }

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

  return { config, pickedComic, currentTabName, progresses, covers, loadCover, searchResult, theme, setTheme }
})
