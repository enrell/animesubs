import { computed, type Ref } from 'vue'
import type { SelectedFile } from '../types/domain'
import type { FileOutcome } from './useTranslationJob'

export type QueueRowState =
  | 'probing'
  | 'ready'
  | 'no-subs'
  | 'queued'
  | 'translating'
  | 'done'
  | 'failed'

interface UseQueueStatesParams {
  selectedFiles: Ref<SelectedFile[]>
  isTranslating: Ref<boolean>
  batchPaths: Ref<string[]>
  currentFileIndex: Ref<number>
  fileOutcomes: Ref<Record<string, FileOutcome>>
  activeFilePaths: Ref<Set<string>>
}

export const useQueueStates = ({
  selectedFiles,
  isTranslating,
  batchPaths,
  currentFileIndex,
  fileOutcomes,
  activeFilePaths
}: UseQueueStatesParams) => {
  const getRowState = (file: SelectedFile): QueueRowState => {
    if (file.loading) return 'probing'
    if (file.error) return 'failed'
    const outcome = fileOutcomes.value[file.path]
    if (outcome) return outcome.status
    if (isTranslating.value) {
      const idx = batchPaths.value.indexOf(file.path)
      if (idx > -1) {
        if (activeFilePaths.value.has(file.path)) return 'translating'
        if (idx >= currentFileIndex.value) return 'queued'
      }
    }
    if (!file.videoInfo) return 'failed'
    if (file.videoInfo.subtitle_tracks.length === 0) return 'no-subs'
    return 'ready'
  }

  const queuePosition = (file: SelectedFile): number => {
    const idx = batchPaths.value.indexOf(file.path)
    if (idx === -1 || idx <= currentFileIndex.value) return 0
    return idx - currentFileIndex.value
  }

  const counts = computed(() => {
    const result = {
      ready: 0,
      queued: 0,
      translating: 0,
      done: 0,
      failed: 0,
      noSubs: 0,
      probing: 0
    }
    for (const file of selectedFiles.value) {
      const state = getRowState(file)
      if (state === 'no-subs') result.noSubs++
      else result[state]++
    }
    return result
  })

  const failedPaths = computed(() =>
    selectedFiles.value.filter(f => getRowState(f) === 'failed' && !f.error).map(f => f.path)
  )

  return { getRowState, queuePosition, counts, failedPaths }
}
