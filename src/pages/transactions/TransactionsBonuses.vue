<script setup lang="ts">
import { BonusPrograms } from '@/widgets/bonus-programs'
import { CashbackBonuses } from '@/widgets/cashback-bonuses'

/** Bonuses and cashback: the programmes, then what the cards brought back. */
</script>

<template>
  <div class="grid gap-6">
    <BonusPrograms />
    <div data-tour="bonuses-cashback"><CashbackBonuses /></div>
  </div>
</template>
