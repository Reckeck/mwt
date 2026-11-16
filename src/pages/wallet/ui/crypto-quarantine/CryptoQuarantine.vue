<script setup lang="ts">
import { computed, ref } from 'vue'

import { CryptoTokenReviewDialog, useTokenReview } from '@/features/crypto-token-review'
import { type QuarantineRow, QuarantineBlock, QuarantineWhyDialog, quarantineRows, useCryptoTokens } from '@/entities/crypto-token'

/**
 * The quarantine under an address's assets (mockups 03 to 05): the tokens that came by themselves
 * and wait for the user. Nothing is drawn when there are none. The holdings come with the account,
 * what a token is (symbol, warnings) from the token list.
 */
const props = defineProps<{ holdings: QuarantinedHolding[] }>()
const tokens = useCryptoTokens()
const { reject } = useTokenReview()

const rows = computed(() => quarantineRows(props.holdings, tokens.data.value ?? []))
const reviewOpen = ref(false)
const whyOpen = ref(false)
const picked = ref<QuarantineRow | null>(null)
function recognize(row: QuarantineRow) {
  picked.value = row
  reviewOpen.value = true
}
</script>

<template>
  <template v-if="rows.length">
    <QuarantineBlock :rows="rows" :busy="reject.isPending.value" @recognize="recognize" @hide="(row) => reject.mutate(row)" @why="whyOpen = true" />
    <CryptoTokenReviewDialog v-model:open="reviewOpen" :row="picked" />
    <QuarantineWhyDialog v-model:open="whyOpen" />
  </template>
</template>
