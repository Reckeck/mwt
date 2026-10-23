<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Building2, ChevronRight, Repeat, Search, User } from 'lucide-vue-next'

import { contactNets } from '@/entities/contact'
import { Button } from '@/shared/ui/button'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { EmptyState } from '@/shared/ui/empty-state'
import { Input } from '@/shared/ui/input'
import { cn } from '@/shared/lib/utils'
import { ContactDebtPhrase } from '../contact-debt-phrase'

/**
 * The contacts (Ops-1 · 6): one flat list, each with its debt in words, so
 * who owes whom reads without opening anyone. Search finds by name, group or
 * any requisite number; a right click acts without selecting.
 */
const props = defineProps<{
  contacts: Contact[]
  /** How many contacts there are at all: «nothing found» and «no contacts yet» read differently. */
  total: number
  balances: ContactBalance[]
  loaded: boolean
  selectedId: string | null
  actionsOf: (contact: Contact) => UiAction[]
  /** On a phone a row opens a screen of its own: an icon tile and a chevron, rows divided. */
  phone?: boolean
}>()
const emit = defineEmits<{ select: [id: string] }>()
const search = defineModel<string>('search', { required: true })

const { t } = useI18n()

const ICONS: Record<ContactKind, typeof User> = { person: User, company: Building2, exchanger: Repeat }
/** Without debts a person reads «долгов нет», a company or an exchanger its kind. */
const emptyText = (contact: Contact) =>
  contact.kind === 'person' ? t('contacts.debt.none') : t(`contactKind.${contact.kind}`).toLowerCase()
</script>

<template>
  <aside :class="cn('grid content-start gap-2 rounded-xl border bg-card', props.phone ? 'px-3' : 'p-2')" data-tour="contact-groups">
    <div v-if="props.total > 5 || search" class="relative">
      <Search class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
      <Input v-model="search" class="h-9 pl-8" :placeholder="t('contacts.search')" :aria-label="t('contacts.search')" />
    </div>

    <EmptyState
      v-if="props.loaded && !props.contacts.length"
      compact
      :title="props.total ? t('contacts.nothingFound') : t('contacts.empty')"
    />
    <ul :class="cn('grid', props.phone ? 'divide-y' : 'gap-0.5')">
      <li v-for="contact in props.contacts" :key="contact.id">
        <ContextMenu :actions="props.actionsOf(contact)">
          <Button
            variant="ghost"
            :class="
              cn(
                'h-auto min-h-[52px] w-full justify-start gap-3 py-2 text-left font-normal',
                props.phone ? 'rounded-none px-0 hover:bg-transparent' : 'px-3',
                props.selectedId === contact.id && !props.phone && 'bg-accent',
                contact.archived && 'opacity-60',
              )
            "
            :aria-current="props.selectedId === contact.id ? 'true' : undefined"
            @click="emit('select', contact.id)"
          >
            <span
              v-if="props.phone"
              class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-secondary text-muted-foreground"
            >
              <component :is="ICONS[contact.kind]" class="size-4" />
            </span>
            <component :is="ICONS[contact.kind]" v-else class="size-4 shrink-0 text-muted-foreground" />
            <span class="flex min-w-0 flex-1 flex-col">
              <span class="truncate text-sm font-medium">{{ contact.name }}</span>
              <span class="truncate text-xs text-muted-foreground">
                <ContactDebtPhrase :nets="contactNets(props.balances, contact.id)" :empty="emptyText(contact)" />
              </span>
            </span>
            <ChevronRight v-if="props.phone" class="size-4 shrink-0 text-muted-foreground" />
          </Button>
        </ContextMenu>
      </li>
    </ul>
  </aside>
</template>
