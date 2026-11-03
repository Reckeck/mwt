<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { isQuickContact } from '@/entities/contact'
import { Badge } from '@/shared/ui/badge'
import { InitialsAvatar } from '@/shared/ui/initials-avatar'
import { cn } from '@/shared/lib/utils'

/** Avatar, name and the «not filled in» badge of a contact made by name only. */
const props = defineProps<{ contact: Contact; selected?: boolean }>()
const { t } = useI18n()
</script>

<template>
  <span class="flex min-w-0 items-center gap-3">
    <InitialsAvatar :name="props.contact.name" />
    <span :class="cn('truncate', props.selected ? 'font-semibold' : 'font-medium')">{{ props.contact.name }}</span>
    <Badge v-if="isQuickContact(props.contact) && !props.contact.archived" variant="secondary" class="shrink-0 font-normal">
      {{ t('contacts.table.incomplete') }}
    </Badge>
  </span>
</template>
