import { ref, type Ref, toValue, type MaybeRefOrGetter } from 'vue'
import { useEventListener } from '@vueuse/core'

import { looksLikeLink } from '@/entities/attachment'

interface CatchHandlers {
  files: (files: File[]) => void
  link: (url: string) => void
}

const hasFiles = (event: DragEvent) => Array.from(event.dataTransfer?.types ?? []).includes('Files')

/**
 * Where files come from besides «Добавить» (Attach 3): dragged onto the card
 * — it lights up while a file is over it — or pasted with Ctrl V outside a
 * field. A pasted address opens the link form with it.
 */
export function useAttachmentCatch(
  target: MaybeRefOrGetter<HTMLElement | null | undefined>,
  active: MaybeRefOrGetter<boolean>,
  handlers: CatchHandlers,
): { dragging: Ref<boolean> } {
  const dragging = ref(false)
  // dragenter/dragleave fire for every child the file passes over; a count says when it really left.
  let depth = 0

  useEventListener(
    () => toValue(target),
    'dragenter',
    (event: DragEvent) => {
      if (!toValue(active) || !hasFiles(event)) return
      event.preventDefault()
      depth += 1
      dragging.value = true
    },
  )
  useEventListener(
    () => toValue(target),
    'dragover',
    (event: DragEvent) => {
      if (!toValue(active) || !hasFiles(event)) return
      event.preventDefault()
      if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy'
    },
  )
  useEventListener(
    () => toValue(target),
    'dragleave',
    () => {
      depth = Math.max(0, depth - 1)
      if (!depth) dragging.value = false
    },
  )
  useEventListener(
    () => toValue(target),
    'drop',
    (event: DragEvent) => {
      depth = 0
      dragging.value = false
      const files = Array.from(event.dataTransfer?.files ?? [])
      if (!toValue(active) || !files.length) return
      event.preventDefault()
      handlers.files(files)
    },
  )

  useEventListener(document, 'paste', (event: ClipboardEvent) => {
    if (!toValue(active)) return
    const element = event.target as HTMLElement | null
    if (element?.closest('input, textarea, [contenteditable]')) return
    const files = Array.from(event.clipboardData?.files ?? [])
    if (files.length) {
      event.preventDefault()
      handlers.files(files)
      return
    }
    const text = event.clipboardData?.getData('text')?.trim() ?? ''
    if (!looksLikeLink(text)) return
    event.preventDefault()
    handlers.link(text)
  })

  return { dragging }
}
