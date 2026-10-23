<script setup lang="ts">
import { BonusPrograms } from '@/widgets/bonus-programs'
import { CashbackBonuses } from '@/widgets/cashback-bonuses'

/** Bonuses and cashback (Ops-2 · 12): the programmes, then what the cards brought back. */
</script>

<template>
  <div class="grid grid-cols-1 gap-6">
    <BonusPrograms add-to="#tx-section-end" />
    <div data-tour="bonuses-cashback"><CashbackBonuses /></div>
  </div>
</template>
