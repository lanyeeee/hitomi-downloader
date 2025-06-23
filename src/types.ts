import { DownloadTaskEvent } from './bindings.ts'

export type CurrentTabName = 'search' | 'downloaded' | 'comic'

export type ProgressData = Extract<DownloadTaskEvent, { event: 'Create' }>['data'] & {
  percentage: number
  indicator: string
}
