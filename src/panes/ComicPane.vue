<script setup lang="ts">
import { useStore } from '../store.ts'
import { Comic, commands } from '../bindings.ts'
import { computed, watch, ref, nextTick } from 'vue'
import { useI18n } from '../utils.ts'
import DownloadButton from '../components/DownloadButton.vue'
import { PhArrowClockwise } from '@phosphor-icons/vue'
import ComicCard from '../components/ComicCard.vue'

const { t } = useI18n()

defineProps<{
  search: (query: string, pageNum: number) => Promise<void>
}>()

const store = useStore()

const relatedComics = ref<Comic[]>([])
const containerRef = ref<HTMLElement>()

const cover = computed<string | undefined>(() =>
  store.pickedComic ? store.covers.get(store.pickedComic.id) : undefined,
)

watch(
  () => store.pickedComic,
  () => {
    if (store.pickedComic === undefined) {
      return
    }
    if (containerRef.value !== undefined) {
      nextTick(() => containerRef.value?.scrollTo({ top: 0, behavior: 'instant' }))
    }

    if (cover.value === undefined) {
      store.loadCover(store.pickedComic.id, store.pickedComic.coverUrl)
    }

    reloadRelatedComics()
  },
)

async function pickComic(id: number) {
  const result = await commands.getComic(id)
  if (result.status === 'error') {
    console.error(result.error)
    return
  }
  store.pickedComic = result.data
}

async function reloadRelatedComics() {
  if (store.pickedComic === undefined) {
    return
  }

  relatedComics.value = []

  const promises = store.pickedComic.related.map(async (id) => {
    const result = await commands.getComic(id)
    if (result.status === 'error') {
      console.error(result.error)
      return
    }
    relatedComics.value.push(result.data)
  })

  await Promise.all(promises)
}

async function showComicDownloadDirInFileManager() {
  if (store.pickedComic === undefined || store.config === undefined) {
    return
  }

  const comicDownloadDir = store.pickedComic.comicDownloadDir
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
  <n-empty
    class="p-2"
    v-if="store.pickedComic === undefined"
    :description="t('comic_pane.empty_description')"></n-empty>
  <div ref="containerRef" v-else class="h-full min-h-0 flex flex-col overflow-auto box-border p-2">
    <div class="font-bold text-xl leading-snug" v-html="store.pickedComic.title" />
    <div class="mt-2 flex w-full flex-col gap-3 sm:flex-row">
      <img
        v-if="cover !== undefined"
        class="w-40 sm:w-44 shrink-0 self-center sm:self-start object-cover rounded shadow-md"
        :src="cover"
        alt="" />
      <n-icon v-else size="50" class="w-40 sm:w-44 h-52 shrink-0 self-center sm:self-start flex items-center justify-center rounded border border-gray-200 dark:border-[#3b3f4d] bg-gray-50 dark:bg-[#181c28]">
        <PhArrowClockwise
          class="cursor-pointer transition-transform duration-500 hover:rotate-360"
          @click="store.loadCover(store.pickedComic.id, store.pickedComic.coverUrl)" />
      </n-icon>
      <div class="flex min-w-0 flex-1 flex-col gap-2 sm:max-w-xl">
        <div class="text-sm opacity-80">
          <!-- TODO: format the date with i18n -->
          {{ store.pickedComic.date }}
        </div>
        <div class="flex text-sm">
          <div>ID: {{ store.pickedComic.id }}</div>
          <div class="ml-auto">{{ store.pickedComic.files.length }} {{ t('search_pane.pages') }}</div>
        </div>
        <div v-if="store.pickedComic.languages.length > 0" class="flex flex-col gap-1">
          <div class="font-bold">{{ t('comic_pane.other_language') }}</div>
          <div class="flex flex-wrap gap-1">
            <n-button
              v-for="(language, index) in store.pickedComic.languages"
              :key="index"
              size="tiny"
              round
              @click="pickComic(language.galleryid)">
              {{ language.language_localname }}
            </n-button>
          </div>
        </div>
        <div class="mt-auto flex flex-wrap items-center gap-2 pt-1">
          <n-button v-if="store.pickedComic.isDownloaded" size="small" @click="showComicDownloadDirInFileManager">
            {{ t('common.open_directory') }}
          </n-button>
          <download-button
            size="small"
            type="primary"
            :comic-id="store.pickedComic.id"
            :comic-downloaded="store.pickedComic.isDownloaded === true" />
        </div>
      </div>
    </div>
    <div class="flex flex-col gap-2 mt-3 pb-2">
      <div v-if="store.pickedComic.artists.length > 0">
        <div class="font-bold">{{ t('common.artist') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="(artist, index) in store.pickedComic.artists"
            :key="index"
            size="tiny"
            round
            @click="search(`artist:${artist.replace(' ', '_')}`, 1)">
            {{ artist }}
          </n-button>
        </div>
      </div>
      <div v-if="store.pickedComic.groups.length > 0">
        <div class="font-bold">{{ t('common.group') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="(group, index) in store.pickedComic.groups"
            :key="index"
            size="tiny"
            round
            @click="search(`group:${group.replace(' ', '_')}`, 1)">
            {{ group }}
          </n-button>
        </div>
      </div>
      <div v-if="store.pickedComic.type !== ''">
        <div class="font-bold">{{ t('common.type') }}</div>
        <n-button size="tiny" round @click="search(`type:${store.pickedComic.type.replace(' ', '_')}`, 1)">
          {{ store.pickedComic.type }}
        </n-button>
      </div>
      <div v-if="store.pickedComic.language !== ''">
        <div class="font-bold">{{ t('common.language') }}</div>
        <n-button
          size="tiny"
          round
          @click="search(`language:${store.pickedComic.language.replace(' ', '_')}`, 1)">
          {{ store.pickedComic.languageLocalname }}
        </n-button>
      </div>
      <div v-if="store.pickedComic.parodys.length > 0">
        <div class="font-bold">{{ t('common.series') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="(series, index) in store.pickedComic.parodys"
            :key="index"
            round
            size="tiny"
            @click="search(`series:${series.replace(' ', '_')}`, 1)">
            {{ series }}
          </n-button>
        </div>
      </div>
      <div v-if="store.pickedComic.characters.length > 0">
        <div class="font-bold">{{ t('common.character') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="(character, index) in store.pickedComic.characters"
            :key="index"
            round
            size="tiny"
            @click="search(`character:${character.replace(' ', '_')}`, 1)">
            {{ character }}
          </n-button>
        </div>
      </div>
      <div v-if="store.pickedComic.tags.length > 0">
        <div class="font-bold">{{ t('common.tag') }}</div>
        <div class="flex flex-wrap gap-1">
          <n-button
            v-for="({ tag, female, male }, index) in store.pickedComic.tags"
            :key="index"
            round
            :color="female !== 0 ? '#F472B6' : male !== 0 ? '#60A5FA' : undefined"
            size="tiny"
            @click="search(`${female !== 0 ? 'female' : male !== 0 ? 'male' : 'tag'}:${tag.replace(' ', '_')}`, 1)">
            {{ tag }}
          </n-button>
        </div>
      </div>
      <div v-if="relatedComics.length > 0">
        <div class="font-bold">{{ t('comic_pane.related') }}</div>
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
          <ComicCard v-for="comic in relatedComics" :key="comic.id" :comic="comic" :search="search" />
        </div>
      </div>
    </div>
  </div>
</template>
