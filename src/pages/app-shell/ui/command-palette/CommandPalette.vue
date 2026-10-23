<script setup lang="ts">
import { computed, nextTick, onScopeDispose, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { onClickOutside, useEventListener } from '@vueuse/core'
import { Search } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Command, CommandList } from '@/shared/ui/command'
import { Kbd } from '@/shared/ui/kbd'
import { pushBackLayer } from '@/shared/lib/back-stack'
import { hotkeyLabel, useHotkey } from '@/shared/lib/hotkeys'
import { useIsPhone } from '@/shared/lib/phone'

import PaletteHints from './PaletteHints.vue'
import PaletteInput from './PaletteInput.vue'
import PaletteMore from './PaletteMore.vue'
import PalettePeek from './PalettePeek.vue'
import PaletteSearch, { type PaletteTarget } from './PaletteSearch.vue'
import { usePalettePeek } from './use-palette-peek'

/**
 * Ctrl K (Paths 1 · 7, 2 · 8): a window over the dimmed page — the results
 * on the left, a glance at the highlighted one on the right, the keys below.
 * A phone gets the whole screen with «Отмена». The header keeps the trigger.
 */
const emit = defineEmits<{ quickAdd: []; settings: []; lock: [] }>()

const { t } = useI18n()
const router = useRouter()
const phone = useIsPhone()

const open = ref(false)
const search = ref('')
/** The entity whose actions are listed (→ on it); the rest of the palette waits. */
const actionsFor = ref<PaletteTarget | null>(null)
const input = ref<InstanceType<typeof PaletteInput> | null>(null)
const list = ref<HTMLElement | null>(null)
const panel = ref<HTMLElement | null>(null)
onClickOutside(panel, () => close(), { ignore: ['[data-command-palette-trigger]'] })
/** The listbox keeps the last pick; cleared so the same row can be picked again. */
const picked = ref<string>('')

const peek = usePalettePeek({ open, search, list, phone })

function show() {
  open.value = true
  void nextTick(() => input.value?.focus())
}
function close() {
  open.value = false
  actionsFor.value = null
}
useHotkey('palette', show)
useEventListener(window, 'keydown', (event: KeyboardEvent) => {
  if (open.value && event.key === 'Escape') close()
})

// The system «Back» on a phone closes the open palette first, like any other layer.
let releaseBack: (() => void) | null = null
watch(open, (isOpen) => {
  releaseBack?.()
  releaseBack = isOpen ? pushBackLayer(close) : null
})
onScopeDispose(() => releaseBack?.())

function run(action: () => void) {
  close()
  picked.value = ''
  action()
}
function openFromPeek(target: EntityRef) {
  if (target.kind !== 'category') return
  const month = new Date().toISOString().slice(0, 7)
  run(() => void router.push({ name: 'money', query: { from: month, to: month, category: target.id } }))
}

const width = computed(() => (peek.item.value ? 'w-[min(860px,calc(100vw-2rem))]' : 'w-[min(620px,calc(100vw-2rem))]'))
const TRIGGER_PHONE = { variant: 'ghost', size: 'icon' } as const
const TRIGGER_WIDE = { variant: 'outline', class: 'w-full justify-start gap-2 px-2 font-normal text-muted-foreground' } as const
</script>

<template>
  <div :class="phone ? 'shrink-0' : 'w-40 min-w-0 sm:w-56 xl:w-72'" data-tour="command-palette">
    <Button
      v-bind="phone ? TRIGGER_PHONE : TRIGGER_WIDE"
      :aria-label="phone ? t('palette.title') : undefined"
      data-command-palette-trigger
      @click="show"
    >
      <Search class="size-4 shrink-0" />
      <template v-if="!phone">
        <span class="min-w-0 flex-1 truncate text-left">{{ t('palette.trigger') }}</span>
        <Kbd class="hidden shrink-0 lg:inline-flex">{{ hotkeyLabel('palette') }}</Kbd>
      </template>
    </Button>
  </div>

  <Teleport to="body">
    <template v-if="open">
      <div v-if="!phone" class="fixed inset-0 z-overlay bg-black/35" aria-hidden="true" data-command-palette-backdrop />
      <div
        ref="panel"
        role="dialog"
        :aria-label="t('palette.title')"
        class="fixed z-overlay flex flex-col overflow-hidden bg-popover text-popover-foreground"
        :class="phone ? 'inset-0 pt-(--safe-top) pb-(--safe-bottom)' : `left-1/2 top-[90px] max-h-[calc(100dvh-120px)] -translate-x-1/2 rounded-xl border shadow-2xl ${width}`"
        data-command-palette
        data-open="true"
      >
        <Command v-model="picked" class="min-h-0 flex-1 bg-transparent">
          <div class="flex h-13 shrink-0 items-center gap-2 border-b px-4">
            <PaletteInput ref="input" class="flex-1" :placeholder="t('palette.placeholder')" :label="t('palette.title')" @search="search = $event" />
            <Button v-if="phone" variant="ghost" size="sm" data-command-palette-cancel @click="close">{{ t('actions.cancel') }}</Button>
            <Kbd v-else class="shrink-0">Esc</Kbd>
          </div>
          <div class="flex min-h-0 flex-1" :class="phone && 'flex-col'">
            <div ref="list" class="min-h-0 min-w-0 flex-1 [&_[data-slot=command-item][data-highlighted]]:bg-primary/10" :class="phone && '[&_[data-slot=command-item]]:min-h-11'">
              <CommandList class="h-full" :class="phone ? 'max-h-none' : 'max-h-[min(40rem,calc(100dvh-200px))]'">
                <PaletteSearch v-model:actions-for="actionsFor" :open="open" :run="run" />
                <PaletteMore v-if="search && !actionsFor" :run="run" @quick-add="emit('quickAdd')" @settings="emit('settings')" @lock="emit('lock')" />
              </CommandList>
            </div>
            <aside v-if="peek.item.value" class="shrink-0 bg-popover" :class="phone ? 'max-h-[55dvh] border-t' : 'w-[min(300px,40vw)] border-l'">
              <PalettePeek :item="peek.item.value" @open="openFromPeek" />
            </aside>
          </div>
          <PaletteHints :category="peek.onCategory.value" :phone="phone" />
        </Command>
      </div>
    </template>
  </Teleport>
</template>
