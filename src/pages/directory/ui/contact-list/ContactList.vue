<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown, Search } from 'lucide-vue-next'

import { ContactQuickCreate } from '@/features/contact-edit'
import { type ContactDebtFilter, type ContactSection, contactNets } from '@/entities/contact'
import { Button } from '@/shared/ui/button'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { EmptyState } from '@/shared/ui/empty-state'
import { Input } from '@/shared/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { formatMoney, signClass } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

/** The contacts: quick add, search, the debt filter, and the list in collapsible sections. */
const props = defineProps<{
  sections: ContactSection[]
  /** How many contacts there are at all: «nothing found» and «no contacts yet» read differently. */
  total: number
  balances: ContactBalance[]
  loaded: boolean
  selectedId: string | null
  actionsOf: (contact: Contact) => UiAction[]
}>()
const emit = defineEmits<{ select: [id: string] }>()
const search = defineModel<string>('search', { required: true })
const filter = defineModel<ContactDebtFilter>('filter', { required: true })

const { t } = useI18n()
const collapsed = ref<string[]>([])
const nets = (id: string) => contactNets(props.balances, id)

function toggleSection(key: string) {
  collapsed.value = collapsed.value.includes(key) ? collapsed.value.filter((entry) => entry !== key) : [...collapsed.value, key]
}
</script>

<template>
  <aside class="grid gap-2">
    <ContactQuickCreate @created="(contact) => emit('select', contact.id)" />
    <div class="relative">
      <Search class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
      <Input v-model="search" class="h-9 pl-8" :placeholder="t('contacts.search')" :aria-label="t('contacts.search')" />
    </div>
    <ToggleGroup v-model="filter" type="single" variant="outline" size="sm" class="w-full" :aria-label="t('contacts.filter')">
      <ToggleGroupItem value="all" class="flex-1">{{ t('contacts.filterAll') }}</ToggleGroupItem>
      <ToggleGroupItem value="owe_me" class="flex-1">{{ t('contacts.filterOweMe') }}</ToggleGroupItem>
      <ToggleGroupItem value="i_owe" class="flex-1">{{ t('contacts.filterIOwe') }}</ToggleGroupItem>
    </ToggleGroup>

    <EmptyState
      v-if="props.loaded && !props.sections.length"
      compact
      :title="props.total ? t('contacts.nothingFound') : t('contacts.empty')"
    />
    <div class="grid gap-2" data-tour="contact-groups">
      <section v-for="section in props.sections" :key="section.key" class="grid gap-0.5">
        <Button
          variant="ghost"
          size="sm"
          class="h-7 justify-between px-3 text-xs font-medium uppercase tracking-wide text-muted-foreground"
          :aria-expanded="!collapsed.includes(section.key)"
          @click="toggleSection(section.key)"
        >
          <span class="flex items-center gap-1">
            <ChevronDown class="size-3.5 transition-transform" :class="collapsed.includes(section.key) && '-rotate-90'" />
            {{ section.title }}
          </span>
          <span class="tabular-nums">{{ section.entries.length }}</span>
        </Button>
        <ul v-show="!collapsed.includes(section.key)" class="grid gap-0.5">
          <li v-for="contact in section.entries" :key="contact.id">
            <ContextMenu :actions="props.actionsOf(contact)">
              <Button
                variant="ghost"
                :class="
                  cn(
                    'h-auto w-full justify-between px-3 py-2 text-left font-normal',
                    props.selectedId === contact.id && 'bg-accent',
                    contact.archived && 'opacity-60',
                  )
                "
                @click="emit('select', contact.id)"
              >
                <span class="min-w-0 truncate font-medium">{{ contact.name }}</span>
                <span class="flex shrink-0 flex-col items-end text-xs tabular-nums">
                  <span v-for="row in nets(contact.id)" :key="row.symbol" :class="signClass(row.net)">
                    {{ formatMoney(row.net, { decimals: 0, currency: row.symbol, signed: true }) }}
                  </span>
                </span>
              </Button>
            </ContextMenu>
          </li>
        </ul>
      </section>
    </div>
  </aside>
</template>
