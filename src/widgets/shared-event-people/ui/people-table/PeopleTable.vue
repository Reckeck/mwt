<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Pin } from 'lucide-vue-next'

import { isQuickContact } from '@/entities/contact'
import { Money } from '@/entities/currency'
import { PersonName, negate, sign } from '@/entities/shared-event'
import { useContacts } from '@/shared/api/queries'
import { cn } from '@/shared/lib/utils'
import { ActionMenu } from '@/shared/ui/action-menu'
import type { UiAction } from '@/shared/ui/context-menu'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

/**
 * Everybody in the event with their share, what they bought and the saldo (green: they are owed,
 * red: they owe). A share set by hand wears a pin. Past eight rows the table scrolls inside itself.
 */
const props = defineProps<{
  people: PersonView[]
  adjustments: ShareOverride[]
  currency: string
  decimals: number
  actionsOf: (person: PersonView) => UiAction[]
}>()
const { t } = useI18n()

const contacts = useContacts()
const quickIds = computed(
  () => new Set((contacts.data.value ?? []).filter((contact) => isQuickContact(contact)).map((contact) => contact.id)),
)
const isFixed = (id: string) => props.adjustments.some((entry) => entry.participant_id === id)
/**
 * The backend's saldo is «owes the organizer» (positive); the table reads from the person's side:
 * what they are owed is green and positive, what they owe is red and negative.
 */
const net = (person: PersonView) => negate(person.saldo)
const netClass = (person: PersonView) =>
  sign(person.saldo) < 0 ? 'text-success' : sign(person.saldo) > 0 ? 'text-destructive' : 'text-muted-foreground'
</script>

<template>
  <Table
    class="tabular-nums"
    container-class="max-h-[26rem] rounded-xl border border-border bg-card shadow-sm [scrollbar-gutter:stable] max-md:max-h-[22rem]"
    data-tour="shared-people-table"
  >
    <TableHeader class="sticky top-0 z-10 bg-card shadow-[0_1px_0_var(--border)]">
      <TableRow>
        <TableHead class="text-xs text-muted-foreground">{{ t('sharedEvents.people.name') }}</TableHead>
        <TableHead class="text-right text-xs text-muted-foreground">{{ t('sharedEvents.people.share') }}</TableHead>
        <TableHead class="text-right text-xs text-muted-foreground max-md:hidden">{{ t('sharedEvents.people.bought') }}</TableHead>
        <TableHead class="text-right text-xs text-muted-foreground max-md:hidden">{{ t('sharedEvents.people.saldo') }}</TableHead>
        <TableHead class="w-12"><span class="sr-only">{{ t('actions.more') }}</span></TableHead>
      </TableRow>
    </TableHeader>
    <TableBody>
      <TableRow v-for="person in props.people" :key="person.id" class="align-top md:align-middle">
        <TableCell class="min-w-0 whitespace-normal px-3 py-2">
          <PersonName
            :name="person.name"
            :kind="person.kind"
            :has-details="person.has_details"
            :quick="person.contact_id ? quickIds.has(person.contact_id) : false"
          />
          <p class="mt-1 pl-11 text-xs text-muted-foreground md:hidden">
            {{ t('sharedEvents.people.boughtShort') }}
            <Money :amount="person.paid" :currency="props.currency" :decimals="props.decimals" :convert="false" />
            ·
            <Money :amount="net(person)" :currency="props.currency" :decimals="props.decimals" :convert="false" signed :class="cn('font-semibold', netClass(person))" />
          </p>
        </TableCell>
        <TableCell class="text-right text-sm">
          <span class="inline-flex items-center justify-end gap-1">
            <Pin v-if="isFixed(person.id)" class="size-3.5 text-muted-foreground" :aria-label="t('sharedEvents.people.fixed')" />
            <Money :amount="person.share" :currency="props.currency" :decimals="props.decimals" :convert="false" />
          </span>
        </TableCell>
        <TableCell class="text-right text-sm max-md:hidden">
          <Money v-if="sign(person.paid) !== 0" :amount="person.paid" :currency="props.currency" :decimals="props.decimals" :convert="false" />
          <span v-else class="text-muted-foreground">—</span>
        </TableCell>
        <TableCell class="text-right text-sm max-md:hidden">
          <Money :amount="net(person)" :currency="props.currency" :decimals="props.decimals" :convert="false" signed :class="cn('font-semibold', netClass(person))" />
        </TableCell>
        <TableCell class="w-12 px-2">
          <ActionMenu :actions="props.actionsOf(person)" :label="t('actions.more')" :sheet="{ title: person.name || t('sharedEvents.me') }" />
        </TableCell>
      </TableRow>
    </TableBody>
  </Table>
</template>
