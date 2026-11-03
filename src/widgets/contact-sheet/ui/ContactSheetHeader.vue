<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { X } from 'lucide-vue-next'

import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import type { UiAction } from '@/shared/ui/context-menu'
import { InitialsAvatar } from '@/shared/ui/initials-avatar'

/**
 * The card's top: avatar, name (the dialog's title, from the `#title` slot), type and group, the close
 * button, and the main action next to «⋯». The reason a transfer cannot be recorded stands under them.
 */
const props = defineProps<{ contact: Contact; menu: UiAction[]; blocked: string | false }>()
const emit = defineEmits<{ close: []; transfer: [] }>()

const { t } = useI18n()
const subtitle = computed(() => [t(`contactKind.${props.contact.kind}`), props.contact.group.trim()].filter(Boolean).join(' · '))
</script>

<template>
  <header class="grid grid-cols-1 gap-3" data-tour="contact-sheet-header">
    <div class="flex items-center gap-3">
      <InitialsAvatar :name="props.contact.name" size="lg" />
      <div class="min-w-0 flex-1">
        <slot name="title" />
        <slot name="description" :text="subtitle" />
      </div>
      <Button variant="ghost" size="icon" :aria-label="t('actions.close')" :title="t('actions.close')" @click="emit('close')">
        <X class="size-5" />
      </Button>
    </div>
    <div class="flex items-center gap-2">
      <Button :disabled="Boolean(props.blocked)" data-tour="contact-transactions" @click="emit('transfer')">
        {{ t('contacts.writeTransfer') }}
      </Button>
      <ActionMenu :actions="props.menu" :label="t('actions.more')" :sheet="{ title: props.contact.name, description: subtitle }" />
    </div>
    <p v-if="props.blocked" class="text-xs text-muted-foreground" data-transfer-blocked>{{ props.blocked }}</p>
  </header>
</template>
