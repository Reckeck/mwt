<script setup lang="ts">
import { useEventListener } from '@vueuse/core'
import { ref, shallowRef, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { buildSelector, hintOf, tableContext, textOf } from '../lib/selector'
import type { PagePick } from './page-pick.types'

/**
 * The page as the background fetch sees it: a sanitized copy (no scripts,
 * forms or frames; the backend adds a CSP) in a frame that may not run
 * code. Same-origin only so the picker can read the element under the
 * pointer or finger; a click marks it and builds its selector.
 */
const props = defineProps<{ html: string; selected: string }>()
const emit = defineEmits<{ pick: [pick: PagePick] }>()
const { t } = useI18n()

const frame = ref<HTMLIFrameElement | null>(null)
const frameDoc = shallowRef<Document | null>(null)
let hovered: Element | null = null

const MARK_STYLE =
  '[data-mwt-hover]{outline:2px solid Highlight!important;cursor:pointer}' +
  '[data-mwt-picked]{outline:3px dashed Highlight!important;outline-offset:2px}' +
  'a{pointer-events:none}'

function mark(doc: Document, selector: string) {
  for (const old of doc.querySelectorAll('[data-mwt-picked]')) old.removeAttribute('data-mwt-picked')
  if (!selector) return
  try {
    doc.querySelector(selector)?.setAttribute('data-mwt-picked', '')
  } catch {
    // A selector typed by hand may be invalid while it is being edited.
  }
}

function attach() {
  const doc = frame.value?.contentDocument ?? null
  hovered = null
  if (doc?.head) {
    const style = doc.createElement('style')
    style.textContent = MARK_STYLE
    doc.head.append(style)
  }
  frameDoc.value = doc
  if (doc) mark(doc, props.selected)
}

useEventListener(frameDoc, 'mouseover', (event: MouseEvent) => {
  hovered?.removeAttribute('data-mwt-hover')
  hovered = event.target as Element | null
  hovered?.setAttribute('data-mwt-hover', '')
})

useEventListener(frameDoc, 'click', (event: MouseEvent) => {
  event.preventDefault()
  const doc = frameDoc.value
  const target = event.target as Element | null
  if (!doc || !target || target === doc.body) return
  const selector = buildSelector(target, doc)
  mark(doc, selector)
  emit('pick', { selector, text: textOf(target), hint: hintOf(target), table: tableContext(target, doc) })
})

watch(
  () => props.selected,
  (selector) => {
    const doc = frame.value?.contentDocument
    if (doc) mark(doc, selector)
  },
)
</script>

<template>
  <div class="grid gap-1.5">
    <p class="text-xs text-muted-foreground">{{ t('sources.picker.hint') }}</p>
    <iframe
      ref="frame"
      class="h-80 w-full rounded-md border bg-background md:h-96"
      sandbox="allow-same-origin"
      referrerpolicy="no-referrer"
      :title="t('sources.picker.title')"
      :srcdoc="props.html"
      data-tour="source-picker"
      @load="attach"
    />
  </div>
</template>
