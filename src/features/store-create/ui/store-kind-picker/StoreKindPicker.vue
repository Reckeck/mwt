<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { getStoreIcon, getSubTypeIcon } from '@/shared/lib/store-icons'

import { useStoreCreate } from '../../model/use-store-create-state'
import { KIND_COLORS, PICK_CARD_CLASS } from '../../store-create.constants'

/** Step 1: a card per wallet kind (top level) or per sub-account type. */
const { t } = useI18n()
const { isSub, creatableKinds, subTypes, pickKind, pickSubType } = useStoreCreate()
</script>

<template>
  <div class="grid gap-2" data-tour="store-create-pick">
    <template v-if="!isSub">
      <Button v-for="info in creatableKinds" :key="info.kind" variant="outline" :class="PICK_CARD_CLASS" @click="pickKind(info.kind)">
        <component :is="getStoreIcon(info.kind)" class="mt-0.5 size-5 shrink-0" :style="{ color: KIND_COLORS[info.kind] }" />
        <span class="grid gap-0.5">
          <span class="text-sm font-medium">{{ t(`stores.kind.${info.kind}`) }}</span>
          <span class="text-xs font-normal text-muted-foreground">{{ t(`stores.kindHint.${info.kind}`) }}</span>
        </span>
      </Button>
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
