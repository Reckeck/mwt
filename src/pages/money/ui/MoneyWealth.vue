<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { Button } from '@/shared/ui/button'
import { PageHeader } from '@/shared/ui/page-header'
import { SectionState } from '@/shared/ui/section-state'
import { AllocationPie } from '@/widgets/allocation-pie'
import { NetWorth, type NetWorthSpan, useNetWorth } from '@/widgets/net-worth'

/** Money · Capital: the same hero and allocation as the accounts screen, with a way to the accounts themselves. */
const { t } = useI18n()
const router = useRouter()
const span = ref<NetWorthSpan>('last12')
const worth = useNetWorth(span)
</script>

<template>
  <div class="grid gap-4">
    <PageHeader :title="t('money.wealth.title')">
      <template #actions>
        <Button variant="outline" data-wealth-wallets @click="router.push({ name: 'stores' })">
          {{ t('glance.capital.wallets') }}
        </Button>
      </template>
    </PageHeader>
    <SectionState
      :loading="worth.loading.value"
      :error="worth.error.value"
      :empty="!worth.hasData.value"
      :empty-title="t('money.wealth.empty')"
      skeleton="chart"
      @retry="worth.refetch()"
    >
      <div class="grid gap-4">
        <NetWorth v-model:span="span" :points="worth.points.value" :currency="worth.currency.value" />
        <AllocationPie :points="worth.points.value" :currency="worth.currency.value" />
      </div>
    </SectionState>
  </div>
</template>
