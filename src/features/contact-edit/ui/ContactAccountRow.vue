<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { onLongPress } from '@vueuse/core'
import { ArrowRight, Copy, Lock, LockOpen, Pencil } from 'lucide-vue-next'

import { maskDetail } from '@/entities/contact'
import { looksLikePhone } from '@/shared/lib/detect-detail'
import { ActionMenu, ActionSheet } from '@/shared/ui/action-menu'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { CHAIN_LABELS } from '@/shared/lib/chains'
import { useCoarsePointer } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'

import { DETAIL_ICONS } from '../contact-edit.constants'
import type { ContactAccountRowProps } from '../contact-edit.types'

/**
 * A requisite as a record (Ops-2 · 7): what it is, whose bank, the number
 * masked, its currency or network. A mouse copies with the icon and edits from
 * «⋯» or the right click; a finger copies with a tap and holds for the menu.
 */
const props = defineProps<ContactAccountRowProps>()
const emit = defineEmits<{ copy: []; transfer: []; edit: []; toggle: []; asPhone: [phone: string] }>()

const { t } = useI18n()
const coarse = useCoarsePointer()

const title = computed(() => props.account.label || t(`detailKind.${props.account.kind}`))
const masked = computed(() => maskDetail(props.account.kind, props.account.value))
/** «RUB», «USDT · TRON»: what the requisite takes. */
const tag = computed(() =>
  [props.account.currency?.toUpperCase(), props.account.network && CHAIN_LABELS[props.account.network].toUpperCase()]
    .filter(Boolean)
    .join(' · '),
)

/** An `other`/`account` requisite that is really a phone: offered once, never rewritten by itself. */
const phoneGuess = computed(() =>
  props.account.kind === 'other' || props.account.kind === 'account' ? looksLikePhone(props.account.value) : null,
)

const actions = computed<UiAction[]>(() => [
  { id: 'copy', label: t('contacts.account.copy'), icon: Copy, run: () => emit('copy') },
  {
    id: 'transfer',
    label: t('contacts.account.transferHere'),
    icon: ArrowRight,
    hidden: !props.account.active,
    disabled: props.transferDisabled,
    run: () => emit('transfer'),
  },
  { id: 'edit', label: t('contacts.account.edit'), icon: Pencil, run: () => emit('edit') },
  {
    id: 'toggle',
    label: props.account.active ? t('contacts.account.deactivate') : t('contacts.account.activate'),
    icon: props.account.active ? Lock : LockOpen,
    run: () => emit('toggle'),
  },
])

const row = ref<HTMLElement | null>(null)
const sheetOpen = ref(false)
let heldAt = 0
onLongPress(
  row,
  () => {
    if (!coarse.value) return
    heldAt = Date.now()
    sheetOpen.value = true
  },
  { delay: 500, modifiers: { stop: false } },
)
/** A tap copies; the finger lifting after a hold does not. */
function tap() {
  if (Date.now() - heldAt > 800) emit('copy')
}
</script>

<template>
  <div>
  <ContextMenu :actions="actions" :disabled="coarse">
    <div
      ref="row"
      :class="cn('flex min-h-[52px] items-center gap-3 py-2', !props.account.active && 'text-muted-foreground')"
      data-contact-account
    >
      <span
        :class="
          cn(
            'flex size-9 shrink-0 items-center justify-center rounded-lg bg-secondary text-muted-foreground',
            !props.account.active && 'opacity-60',
          )
        "
      >
        <component :is="DETAIL_ICONS[props.account.kind]" class="size-4" />
      </span>
      <!-- A finger taps the whole record to copy; a mouse has the icon on the right. -->
      <Button
        v-if="coarse"
        variant="ghost"
        class="h-auto min-w-0 flex-1 justify-start p-0 text-left font-normal hover:bg-transparent"
        @click="tap"
      >
        <span class="flex min-w-0 flex-col">
          <span class="flex min-w-0 items-center gap-1.5">
            <span class="truncate text-sm font-medium">{{ title }}</span>
            <Badge v-if="props.account.group" variant="outline" class="shrink-0 font-normal">{{ props.account.group }}</Badge>
            <Badge v-if="!props.account.active" variant="outline" class="shrink-0 font-normal">{{ t('contacts.inactive') }}</Badge>
          </span>
          <span class="truncate text-xs text-muted-foreground tabular-nums">{{ [masked, tag].filter(Boolean).join(' · ') }}</span>
        </span>
      </Button>
      <div v-else class="flex min-w-0 flex-1 flex-col">
        <span class="flex min-w-0 items-center gap-1.5">
          <span :class="cn('truncate text-sm font-medium', !props.account.active && 'font-normal')">{{ title }}</span>
          <Badge v-if="props.account.group" variant="outline" class="shrink-0 font-normal">{{ props.account.group }}</Badge>
          <Badge v-if="!props.account.active" variant="outline" class="shrink-0 font-normal">{{ t('contacts.inactive') }}</Badge>
        </span>
        <span class="truncate text-xs text-muted-foreground tabular-nums">{{ masked }}</span>
      </div>
      <Badge v-if="tag && !coarse" variant="secondary" :class="cn('shrink-0 font-medium', !props.account.active && 'opacity-60')">
        {{ tag }}
      </Badge>
      <Button
        variant="ghost"
        size="icon-sm"
        class="shrink-0 text-muted-foreground"
        :title="t('contacts.copy')"
        :aria-label="t('contacts.copy')"
        data-contact-copy
        @click="emit('copy')"
      >
        <Copy class="size-4" />
      </Button>
      <ActionMenu v-if="!coarse" :actions="actions" :label="t('actions.more')" />
    </div>
    <p v-if="phoneGuess" class="-mt-1 flex flex-wrap items-center gap-2 pb-2 pl-12 text-xs text-muted-foreground" data-contact-phone-hint>
      <span>{{ t('contacts.account.looksLikePhone') }}</span>
      <Button type="button" variant="outline" size="sm" data-contact-phone-yes @click="emit('asPhone', phoneGuess)">
        {{ t('actions.yes') }}
      </Button>
    </p>
  </ContextMenu>
  <ActionSheet v-if="coarse" v-model:open="sheetOpen" :actions="actions" :title="title" :description="masked" />
  </div>
</template>
