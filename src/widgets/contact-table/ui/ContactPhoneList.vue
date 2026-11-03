<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { isQuickContact } from '@/entities/contact'
import { Button } from '@/shared/ui/button'
import { Badge } from '@/shared/ui/badge'
import { InitialsAvatar } from '@/shared/ui/initials-avatar'
import { cn } from '@/shared/lib/utils'

import type { ContactRow } from '../contact-table.types'
import ContactDebtCell from './ContactDebtCell.vue'

/** The phone's list: name and type on the left, the debt and who owes whom on the right. */
const props = defineProps<{ rows: ContactRow[]; selectedId: string }>()
const emit = defineEmits<{ select: [id: string] }>()

const { t } = useI18n()
const sub = (contact: Contact) => [t(`contactKind.${contact.kind}`), contact.group.trim()].filter(Boolean).join(' · ')
</script>

<template>
  <ul class="divide-y overflow-hidden rounded-xl border bg-card">
    <li v-for="row in props.rows" :key="row.id">
      <Button
        variant="ghost"
        :class="
          cn(
            'h-auto min-h-[60px] w-full justify-start gap-3 rounded-none px-3 py-2 text-left font-normal',
            row.id === props.selectedId && 'bg-accent',
            row.contact.archived && 'opacity-60',
          )
        "
        :aria-current="row.id === props.selectedId ? 'true' : undefined"
        @click="emit('select', row.id)"
      >
        <InitialsAvatar :name="row.contact.name" />
        <span class="flex min-w-0 flex-1 flex-col">
          <span class="flex min-w-0 items-center gap-2">
            <span class="truncate text-sm font-medium">{{ row.contact.name }}</span>
            <Badge v-if="isQuickContact(row.contact) && !row.contact.archived" variant="secondary" class="shrink-0 font-normal">
              {{ t('contacts.table.incomplete') }}
            </Badge>
          </span>
          <span class="truncate text-xs text-muted-foreground">{{ sub(row.contact) }}</span>
        </span>
        <span v-if="row.nets.length" class="flex shrink-0 flex-col items-end text-sm">
          <ContactDebtCell :nets="row.nets" />
          <span class="text-xs text-muted-foreground">
            {{ row.nets[0].net > 0 ? t('contacts.table.owesMe') : t('contacts.table.iOweShort') }}
          </span>
        </span>
      </Button>
    </li>
  </ul>
</template>
