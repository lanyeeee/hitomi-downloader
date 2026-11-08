<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { Comic, commands } from '../bindings.ts'
import { useStore } from '../store.ts'
import { useI18n } from '../utils.ts'
import { PhArrowClockwise } from '@phosphor-icons/vue'

const { t } = useI18n()

const props = defineProps<{
  comic: Comic
  search: (query: string, pageNum: number) => Promise<void>
}>()

const formattedDate = computed(() => {
  if (!props.comic.date) return ''
  const dateObj = new Date(props.comic.date)
  if (isNaN(dateObj.getTime())) return props.comic.date
  return new Intl.DateTimeFormat(navigator.language || 'es-ES', {
    year: 'numeric',
    month: 'short',
    day: 'numeric'
  }).format(dateObj)
})

const store = useStore()

const cover = computed<string | undefined>(() => store.covers.get(props.comic.id))

onMounted(() => {
  if (cover.value !== undefined) {
    return
  }

  store.loadCover(props.comic.id, props.comic.coverUrl)
})

async function pickComic() {
  store.pickedComic = props.comic
  store.currentTabName = 'comic'
}

async function exportCbz() {
  const result = await commands.exportCbz(props.comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
}

async function exportPdf() {
  const result = await commands.exportPdf(props.comic)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
}

async function showComicDownloadDirInFileManager() {
  if (store.config === undefined) {
    return
  }

  const comicDownloadDir = props.comic.comicDownloadDir
  if (comicDownloadDir === undefined || comicDownloadDir === null) {
    console.error('Comic download directory is undefined or null')
    return
  }

  const result = await commands.showPathInFileManager(comicDownloadDir)
  if (result.status === 'error') {
    console.error(result.error)
  }
}
</script>

<template>
  <n-card 
    content-style="padding: 0.5rem;" 
    hoverable 
    class="bg-white hover:bg-gray-50 dark:bg-[#272d3d] dark:hover:bg-[#343b4f] border-gray-200 dark:border-[#3b3f4d] transition-colors duration-200"
  >
    <div class="flex flex-col sm:flex-row h-full gap-4">
      <!-- Miniatura sin zoom ni modal, manteniendo la funcionalidad de clic original -->
      <div class="relative flex-shrink-0 flex justify-center sm:justify-start items-center">
        <img
          v-if="cover !== undefined"
          loading="lazy"
          decoding="async"
          class="w-24 sm:w-28 h-auto object-cover rounded cursor-pointer shadow-md"
          :src="cover"
          alt=""
          @click="pickComic" 
        />
        <n-icon v-else size="50" class="w-24 sm:w-28 h-32 flex items-center justify-center rounded border border-gray-200 dark:border-[#3b3f4d] bg-gray-50 dark:bg-[#181c28]">
          <PhArrowClockwise
            class="cursor-pointer transition-transform duration-500 hover:rotate-360 text-gray-400 dark:text-[#a7acb9]"
            @click="store.loadCover(props.comic.id, props.comic.coverUrl)" 
          />
        </n-icon>
      </div>

      <!-- Contenido principal -->
      <div class="flex flex-col w-full overflow-hidden gap-1.5 min-w-0">
        <!-- Título -->
        <div
          class="font-bold text-lg leading-snug line-clamp-2 cursor-pointer transition-colors duration-200 text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-[#6c7ec6]"
          v-html="comic.title"
          @click="pickComic" 
        />

        <!-- Metadatos estructurados con truncado responsive -->
        <div class="flex flex-col gap-1 text-xs text-gray-500 dark:text-[#a7acb9] min-w-0">
          <!-- Artistas -->
          <div v-if="comic.artists?.length" class="flex items-center gap-1.5 overflow-hidden">
            <span class="font-semibold text-gray-700 dark:text-white whitespace-nowrap flex-shrink-0">{{ t('common.artist') }}:</span>
            <div class="flex items-center gap-1.5 overflow-hidden md:overflow-visible">
              <n-button
                v-for="(artist, index) in comic.artists"
                :key="index"
                size="tiny"
                class="border-gray-200 dark:border-[#3b3f4d] truncate max-w-[150px] md:max-w-none"
                @click="search(`artist:${artist.replace(' ', '_')}`, 1)">
                <span class="truncate">{{ artist }}</span>
              </n-button>
            </div>
          </div>

          <!-- Series -->
          <div v-if="comic.parodys?.length" class="flex items-center gap-1.5 overflow-hidden">
            <span class="font-semibold text-gray-700 dark:text-white whitespace-nowrap flex-shrink-0">{{ t('common.series') }}:</span>
            <div class="flex items-center gap-1.5 overflow-hidden md:overflow-visible">
              <n-button
                v-for="(series, index) in comic.parodys"
                :key="index"
                size="tiny"
                class="border-gray-200 dark:border-[#3b3f4d] truncate max-w-[150px] md:max-w-none"
                @click="search(`series:${series.replace(' ', '_')}`, 1)">
                <span class="truncate">{{ series }}</span>
              </n-button>
            </div>
          </div>

          <!-- Tipo y Lenguaje -->
          <div class="flex flex-wrap items-center gap-4">
            <div v-if="comic.type" class="flex items-center gap-1.5">
              <span class="font-semibold text-gray-700 dark:text-white whitespace-nowrap">{{ t('common.type') }}:</span>
              <n-button size="tiny" class="border-gray-200 dark:border-[#3b3f4d]" @click="search(`type:${comic.type.replace(' ', '_')}`, 1)">
                {{ comic.type }}
              </n-button>
            </div>
            <div v-if="comic.language" class="flex items-center gap-1.5">
              <span class="font-semibold text-gray-700 dark:text-white whitespace-nowrap">{{ t('common.language') }}:</span>
              <n-button size="tiny" class="border-gray-200 dark:border-[#3b3f4d]" @click="search(`language:${comic.language.replace(' ', '_')}`, 1)">
                {{ comic.languageLocalname }}
              </n-button>
            </div>
          </div>

          <!-- Etiquetas (Tags) con truncado estricto en móvil -->
          <div v-if="comic.tags?.length" class="flex items-center gap-1 mt-0.5 overflow-hidden">
            <span class="font-semibold text-gray-700 dark:text-white whitespace-nowrap flex-shrink-0">{{ t('common.tag') }}:</span>
            <div class="flex items-center gap-1 overflow-hidden md:flex-wrap md:overflow-visible">
              <n-button
                v-for="({ tag, female, male }, index) in comic.tags"
                :key="index"
                size="tiny"
                class="border-gray-200 dark:border-[#3b3f4d] text-[11px] truncate max-w-[120px] md:max-w-none"
                @click="search(`${female !== 0 ? 'female' : male !== 0 ? 'male' : 'tag'}:${tag.replace(' ', '_')}`, 1)">
                <span class="truncate">{{ tag }}</span>
              </n-button>
            </div>
          </div>
        </div>

        <!-- Pie de tarjeta -->
        <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between mt-auto pt-2 gap-2 border-t border-gray-200 dark:border-[#3b3f4d]/50 text-xs text-gray-400 dark:text-[#7c818e]">
          <div class="flex items-center gap-3">
            <span class="px-2 py-0.5 rounded bg-gray-100 dark:bg-transparent text-gray-800 dark:text-white font-mono">{{ formattedDate }}</span>
            <span class="px-2 py-0.5 rounded bg-gray-100 dark:bg-transparent text-gray-800 dark:text-white font-mono">{{ comic.files.length }}P</span>
          </div>
          
          <div v-if="comic.isDownloaded === true" class="flex flex-wrap items-center gap-1.5 w-full sm:w-auto justify-end">
            <n-button size="tiny" class="border-gray-200 dark:border-[#3b3f4d]" @click="showComicDownloadDirInFileManager">
              {{ t('common.open_directory') }}
            </n-button>
            <n-button type="primary" size="tiny" class="border-gray-200 dark:border-[#3b3f4d]" @click="exportPdf">
              {{ t('common.export_pdf') }}
            </n-button>
            <n-button type="primary" size="tiny" class="border-gray-200 dark:border-[#3b3f4d]" @click="exportCbz">
              {{ t('common.export_cbz') }}
            </n-button>
          </div>
        </div>
      </div>
    </div>
  </n-card>
</template>