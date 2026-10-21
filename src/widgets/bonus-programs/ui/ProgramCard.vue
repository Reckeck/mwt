<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Minus, Plus, Settings2 } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EntityLink } from '@/shared/ui/entity-link'
import { ActionMenu } from '@/shared/ui/action-menu'
import { formatDate, formatQuantity } from '@/shared/lib/format'

/** One programme: points and their worth, what burns next, how it knows the user, which cards earn. */
const props = defineProps<{
  store: Store
  balance: LoyaltyBalance | null
  baseCurrency: string
  /** Store id → name, for the source cards. */
  names: Map<string, string>
  contactName: string | null
  /** Came here from an old wallet link: the card is marked. */
  highlighted: boolean
}>()
const emit = defineEmits<{ points: [mode: 'credit' | 'spend']; settings: [] }>()
const { t } = useI18n()

const soon = (iso: string | null) => !!iso && new Date(iso).getTime() - Date.now() < 30 * 86_400_000
const actions = [{ id: 'settings', label: t('bonuses.settings'), icon: Settings2, run: () => emit('settings') }]
</script>

<template>
  <Card size="sm" :class="props.highlighted ? 'ring-2 ring-primary' : ''" :data-program="props.store.id">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="truncate font-medium">{{ props.store.name }}</p>
        <ActionMenu :actions="actions" :label="t('bonuses.actions')" />
      </div>
    </template>
    <div class="grid gap-3 text-sm">
      <div v-if="props.store.loyalty" class="flex flex-wrap items-end justify-between gap-3">
        <div class="grid gap-0.5">
          <p class="text-2xl font-semibold tabular-nums">
            {{ formatQuantity(props.balance?.points ?? '0', 0) }}
            <span class="text-sm font-normal text-muted-foreground">{{ props.store.loyalty.points_symbol }}</span>
          </p>
          <p class="text-muted-foreground">
            {{ t('loyalty.worth') }}
            <Money :amount="props.balance?.value_base ?? '0'" :currency="props.baseCurrency" />
          </p>
        </div>
        <div class="flex gap-2">
          <Button size="sm" @click="emit('points', 'credit')"><Plus class="size-4" />{{ t('loyalty.credit') }}</Button>
          <Button size="sm" variant="outline" @click="emit('points', 'spend')">
            <Minus class="size-4" />{{ t('loyalty.spend') }}
          </Button>
        </div>
      </div>
      <p v-else class="text-muted-foreground">{{ t('loyalty.setup') }}</p>
      <p
        v-if="props.balance?.expires_at"
        class="text-xs"
        :class="soon(props.balance.expires_at) ? 'text-warning' : 'text-muted-foreground'"
      >
        {{ t('loyalty.expiring', { points: props.balance.expiring_points ?? '', date: formatDate(props.balance.expires_at) }) }}
      </p>
      <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
        <template v-if="props.store.loyalty?.login">
          <dt class="text-muted-foreground">{{ t('loyalty.login') }}</dt>
          <dd class="truncate">{{ props.store.loyalty.login }}</dd>
        </template>
        <template v-if="props.store.loyalty?.source_store_ids.length">
          <dt class="text-muted-foreground">{{ t('loyalty.sources') }}</dt>
          <dd class="flex flex-wrap gap-1">
            <EntityLink
              v-for="id in props.store.loyalty.source_store_ids"
              :key="id"
              :target="{ kind: 'store', id }"
              :label="props.names.get(id) ?? ''"
            />
          </dd>
        </template>
        <template v-if="props.store.contact_id">
          <dt class="text-muted-foreground">{{ t('stores.contact') }}</dt>
          <dd><EntityLink :target="{ kind: 'contact', id: props.store.contact_id }" :label="props.contactName ?? ''" /></dd>
        </template>
      </dl>
    </div>
  </Card>
</template>
