<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { getStoreIcon, getSubTypeIcon } from '@/shared/lib/store-icons'
import { cn } from '@/shared/lib/utils'

import { useStoreCreate } from '../../model/use-store-create-state'
import { PICK_CARD_CLASS } from '../../store-create.constants'

/**
 * Step 1 (Wallets-1 · 2): a card per wallet kind, two in a row; a click picks,
 * «Далее» or a second click moves on. A sub-account: a card per type, a click moves on.
 */
const { t } = useI18n()
const { isSub, kind, creatableKinds, subTypes, pickKind, pickSubType, goNext } = useStoreCreate()

function choose(value: StoreKind) {
  if (kind.value === value) goNext()
  else pickKind(value)
}
</script>

<template>
  <div class="grid gap-3" data-tour="store-create-pick">
    <template v-if="!isSub">
      <div class="grid gap-2 @md:grid-cols-2">
        <Button
          v-for="info in creatableKinds"
          :key="info.kind"
          variant="outline"
          :aria-pressed="kind === info.kind"
          :data-kind="info.kind"
          :class="cn(PICK_CARD_CLASS, 'flex-col gap-1', kind === info.kind && 'border-primary bg-primary/5 hover:bg-primary/5')"
          @click="choose(info.kind)"
        >
          <span class="flex items-center gap-2 text-sm font-medium">
            <component :is="getStoreIcon(info.kind)" class="size-4 shrink-0" />
            {{ t(`stores.kind.${info.kind}`) }}
          </span>
          <span class="text-xs font-normal text-muted-foreground">{{ t(`stores.kindHint.${info.kind}`) }}</span>
        </Button>
      </div>
      <p class="text-xs text-muted-foreground">{{ t('storeWizard.kindMatters') }}</p>
    </template>
    <template v-else>
      <p v-if="!subTypes.length" class="text-sm text-muted-foreground">{{ t('errors.SUB_TYPE_NOT_ALLOWED') }}</p>
      <Button
        v-for="info in subTypes"
        :key="info.sub_type"
        variant="outline"
        :class="PICK_CARD_CLASS"
        @click="pickSubType(info.sub_type)"
      >
        <component :is="getSubTypeIcon(info.sub_type)" class="mt-0.5 size-5 shrink-0 text-muted-foreground" />
        <span class="grid gap-0.5">
          <span class="text-sm font-medium">{{ t(`subType.${info.sub_type}`) }}</span>
          <span class="text-xs font-normal text-muted-foreground">{{ t(`subTypeHint.${info.sub_type}`) }}</span>
        </span>
      </Button>
    </template>
  </div>
</template>
