<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Link2 } from 'lucide-vue-next'

import { shortAddress } from '@/entities/crypto-account'
import { Money } from '@/entities/currency'
import { formatDate } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

import { relationLocation, relationSections, relationsEmpty } from './relation-sections'
import { useCryptoRelations } from './use-crypto-relations'

/**
 * `Relations` of a wallet the engine drives (spec E15): its other addresses, the exchangers it dealt with,
 * the protocols it used and the fiat records paired with its operations. Every row opens the operations
 * already narrowed to it. Computed from the events, nothing is stored.
 */
const props = defineProps<{ accountId: string }>()

const { t } = useI18n()
const relations = useCryptoRelations(() => props.accountId)
const sections = computed(() => relationSections(relations.data.value))
const empty = computed(() => relationsEmpty(relations.data.value))
const fiat = computed(() => relations.data.value?.fiat ?? { items: [], total: 0 })
const allOperations = computed(() => relationLocation({ account_ids: [props.accountId], show_hidden: false }))

const ROW =
  'flex min-h-10 min-w-0 items-center justify-between gap-3 rounded-md px-2 py-1.5 text-sm hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring'
</script>

<template>
  <Card data-tour="crypto-relations">
    <SectionState
      :loading="relations.isPending.value"
      :error="relations.error.value"
      :empty="empty"
      :empty-title="t('cryptoTrack.relations.emptyTitle')"
      :empty-description="t('cryptoTrack.relations.emptyHint')"
      :empty-icon="Link2"
      compact
      @retry="relations.refetch()"
    >
      <template #empty>
        <Button as-child variant="outline">
          <RouterLink :to="allOperations">{{ t('cryptoTrack.relations.openOperations') }}</RouterLink>
        </Button>
      </template>
      <div class="grid gap-4">
        <section v-for="section in sections" :key="section.group" class="grid gap-1" :data-relations="section.group">
          <h3 class="text-xs font-medium text-muted-foreground">{{ t(`cryptoTrack.relations.group.${section.group}`) }}</h3>
          <ul class="grid gap-0.5">
            <li v-for="row in section.rows" :key="row.key">
              <RouterLink :to="row.to" :class="ROW" data-relation-row>
                <span class="min-w-0">
                  <span class="block truncate font-medium">{{ row.label }}</span>
                  <span v-if="row.address" class="block truncate text-xs text-muted-foreground">{{ shortAddress(row.address) }}</span>
                </span>
                <span class="shrink-0 text-right text-xs text-muted-foreground tabular-nums">
                  {{ t('cryptoTrack.relations.count', { count: row.count }) }}
                  <span class="block">{{ formatDate(row.lastTime) }}</span>
                </span>
              </RouterLink>
            </li>
          </ul>
          <p v-if="section.total > section.rows.length" class="text-xs text-muted-foreground" data-relations-capped>
            {{ t('entity.more', { count: section.total - section.rows.length }) }}
          </p>
        </section>
        <section v-if="fiat.items.length" class="grid gap-1" data-relations="fiat">
          <h3 class="text-xs font-medium text-muted-foreground">{{ t('cryptoTrack.relations.group.fiat') }}</h3>
          <ul class="grid gap-0.5">
            <li v-for="row in fiat.items" :key="row.event_id">
              <RouterLink :to="relationLocation(row.filter, row.event_id)" :class="ROW" data-relation-row>
                <span class="min-w-0">
                  <span class="block truncate font-medium">{{ row.fiat.place || row.fiat.comment }}</span>
                  <span v-if="row.fiat.place && row.fiat.comment" class="block truncate text-xs text-muted-foreground">{{ row.fiat.comment }}</span>
                </span>
                <span class="shrink-0 text-right text-xs text-muted-foreground tabular-nums">
                  <Money :amount="row.fiat.amount" :currency="row.fiat.currency" />
                  <span class="block">{{ formatDate(row.fiat.time) }}</span>
                </span>
              </RouterLink>
            </li>
          </ul>
          <p v-if="fiat.total > fiat.items.length" class="text-xs text-muted-foreground" data-relations-capped>
            {{ t('entity.more', { count: fiat.total - fiat.items.length }) }}
          </p>
        </section>
      </div>
    </SectionState>
  </Card>
</template>
