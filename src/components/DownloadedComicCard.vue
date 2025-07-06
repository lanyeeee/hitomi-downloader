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
  <n-card content-style="padding: 0.25rem;" hoverable>
    <div class="flex h-full">
      <img
        v-if="cover !== undefined"
        class="w-24 object-contain mr-4 cursor-pointer transition-transform duration-200 hover:scale-108"
        :src="cover"
        alt=""
        @click="pickComic" />
      <n-icon v-else size="50" class="w-28 h-full flex items-center justify-center flex-shrink-0">
        <PhArrowClockwise
          class="cursor-pointer transition-transform duration-500 hover:rotate-360"
          @click="store.loadCover(props.comic.id, props.comic.coverUrl)" />
      </n-icon>
      <div class="flex flex-col w-full overflow-hidden gap-row-1">
        <div
          class="font-bold text-xl line-clamp-2 cursor-pointer transition-colors duration-200 hover:text-blue-5"
          v-html="comic.title"
          @click="pickComic" />
        <div class="flex items-center gap-col-1">
          <div class="whitespace-nowrap">{{ t('common.artist') }}</div>
          <n-button
            v-for="(artist, index) in comic.artists"
            :key="index"
            size="tiny"
            @click="search(`artist:${artist.replace(' ', '_')}`, 1)">
            {{ artist }}
          </n-button>
        </div>
        <div class="flex items-center gap-col-1">
          <div class="whitespace-nowrap">{{ t('common.series') }}</div>
          <n-button
            v-for="(series, index) in comic.parodys"
            :key="index"
            size="tiny"
            @click="search(`series:${series.replace(' ', '_')}`, 1)">
            {{ series }}
          </n-button>
        </div>
        <div class="flex items-center gap-col-1">
          <div class="whitespace-nowrap">{{ t('common.type') }}</div>
          <n-button size="tiny" @click="search(`type:${comic.type.replace(' ', '_')}`, 1)">
            {{ comic.type }}
          </n-button>
        </div>
        <div class="flex items-center gap-col-1">
          <div class="whitespace-nowrap">{{ t('common.language') }}</div>
          <n-button
            v-if="comic.language !== ''"
            size="tiny"
            @click="search(`language:${comic.language.replace(' ', '_')}`, 1)">
            {{ comic.languageLocalname }}
          </n-button>
        </div>
        <div class="flex items-center gap-col-1">
          <div class="whitespace-nowrap">{{ t('common.tag') }}</div>
          <n-button
            v-for="({ tag, female, male }, index) in comic.tags"
            :key="index"
            size="tiny"
            @click="search(`${female !== 0 ? 'female' : male !== 0 ? 'male' : 'tag'}:${tag.replace(' ', '_')}`, 1)">
            {{ tag }}
          </n-button>
        </div>
        <div class="flex items-center gap-col-1">
          <!-- TODO: format the date with i18n -->
          <div>{{ comic.date }}</div>
          <div class="ml-auto">{{ comic.files.length }}P</div>
        </div>
        <div v-if="comic.isDownloaded === true" class="flex mt-auto gap-1">
          <n-button size="tiny" @click="showComicDownloadDirInFileManager">
            {{ t('common.open_directory') }}
          </n-button>
          <n-button type="primary" class="ml-auto" size="tiny" @click="exportPdf">
            {{ t('common.export_pdf') }}
          </n-button>
          <n-button type="primary" size="tiny" @click="exportCbz">
            {{ t('common.export_cbz') }}
          </n-button>
        </div>
      </div>
    </div>
  </n-card>
</template>
