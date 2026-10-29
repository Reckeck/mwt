<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { ME, personStatus, sumDecimals } from '@/entities/shared-event'
import { formatDayMonthShort } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Badge } from '@/shared/ui/badge'
import type { UiAction } from '@/shared/ui/context-menu'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Table, TableBody, TableCell, TableRow } from '@/shared/ui/table'

import { personLine } from '../../lib/person-line'
import { PersonDebt } from '../person-debt'
import { STATUS_BADGE } from './settlement-table.constants'

/**
 * `Who owes whom`: everyone's share, what they bought and what is left between them and the organizer.
 * A row has its main button (`received` / `Gave` / `Pay my share`) and «⋯» for the rest.
 * The organizer's own row is left out when it is me: everything is counted against me.
 */
const props = defineProps<{
  view: SharedEventView
  primaryOf: (person: PersonView) => { label: string } | null
  actionsOf: (person: PersonView) => UiAction[]
}>()
const emit = defineEmits<{ primary: [person: PersonView] }>()

const { t } = useI18n()

const currency = computed(() => props.view.event.currency)
const decimals = computed(() => sumDecimals(props.view))
const organizer = computed(() => props.view.people.find((person) => person.is_organizer))
const rows = computed(() => props.view.people.filter((person) => !(person.is_organizer && person.id === ME)))
const viaName = computed(() =>
  organizer.value?.id === ME ? t('sharedEvents.settle.viaMe') : (organizer.value?.name ?? ''),
)

/** Everything a row shows, worked out once per refresh. */
const models = computed(() =>
  rows.value.map((person) => {
    const status = personStatus(person)
    return {
      person,
      name: person.name || t('sharedEvents.me'),
      line: personLine(props.view, person),
      badge:
        status === 'organizer' || status === 'even'
          ? null
          : { ...STATUS_BADGE[status], label: t(`sharedEvents.settle.status.${status}`) },
      primary: props.primaryOf(person),
      actions: props.actionsOf(person),
      due: person.due_at ? t('sharedEvents.due.until', { date: formatDayMonthShort(person.due_at) }) : '',
    }
  }),
)
</script>

<template>
  <Card data-tour="shared-settlement">
    <template #header>
      <div class="flex flex-wrap items-baseline gap-x-2">
        <h3 class="text-base font-semibold">{{ t('sharedEvents.settle.who') }}</h3>
        <span class="text-xs text-muted-foreground">{{ t('sharedEvents.settle.via', { name: viaName }) }}</span>
      </div>
    </template>
    <Table>
      <TableBody>
        <TableRow v-for="row in models" :key="row.person.id" class="align-top md:align-middle">
          <TableCell class="w-auto min-w-0 whitespace-normal px-2 py-3 md:w-[26%]">
            <div class="flex min-w-0 items-start gap-3 md:items-center">
              <span
                class="flex size-8 shrink-0 items-center justify-center rounded-md bg-secondary text-sm text-muted-foreground"
                aria-hidden="true"
              >
                {{ row.name.slice(0, 1).toUpperCase() }}
              </span>
              <div class="grid min-w-0 gap-0.5">
                <span class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
                  <span class="truncate font-medium">{{ row.name }}</span>
                  <Badge v-if="row.person.kind === 'guest'" variant="outline" class="font-normal">
                    {{ t('sharedEvents.guest') }}
                  </Badge>
                  <Badge v-if="row.badge" :variant="row.badge.variant" class="md:hidden">{{ row.badge.label }}</Badge>
                </span>
                <span class="text-xs text-muted-foreground tabular-nums md:hidden">
                  <i18n-t keypath="sharedEvents.settle.shareBought" tag="span">
                    <template #share><Money :amount="row.person.share" :currency="currency" :decimals="decimals" :convert="false" /></template>
                    <template #paid><Money :amount="row.person.paid" :currency="currency" :decimals="decimals" :convert="false" /></template>
                  </i18n-t>
                </span>
                <span class="text-sm md:hidden">
                  <PersonDebt :line="row.line" :currency="currency" :decimals="decimals" />
                  <span v-if="row.due" class="ml-1 text-xs text-muted-foreground">{{ row.due }}</span>
                </span>
              </div>
            </div>
          </TableCell>
          <TableCell class="text-xs text-muted-foreground tabular-nums max-md:hidden">
            <i18n-t keypath="sharedEvents.settle.shareBought" tag="span">
              <template #share><Money :amount="row.person.share" :currency="currency" :decimals="decimals" :convert="false" /></template>
              <template #paid><Money :amount="row.person.paid" :currency="currency" :decimals="decimals" :convert="false" /></template>
            </i18n-t>
          </TableCell>
          <TableCell class="text-sm max-md:hidden">
            <PersonDebt :line="row.line" :currency="currency" :decimals="decimals" />
            <span v-if="row.due" class="ml-1 text-xs text-muted-foreground">{{ row.due }}</span>
          </TableCell>
          <TableCell class="w-28 max-md:hidden">
            <Badge v-if="row.badge" :variant="row.badge.variant">{{ row.badge.label }}</Badge>
          </TableCell>
          <TableCell class="w-px px-2 py-2">
            <div class="flex items-center justify-end gap-2">
              <Button v-if="row.primary" variant="outline" size="sm" class="whitespace-nowrap" @click="emit('primary', row.person)">
                {{ row.primary.label }}
              </Button>
              <ActionMenu
                :actions="row.actions"
                :label="t('actions.more')"
                :class="cn(!row.actions.length && 'invisible')"
                :sheet="{ title: row.name }"
              />
            </div>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </Card>
</template>
