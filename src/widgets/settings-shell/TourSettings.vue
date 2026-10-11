<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { GraduationCap } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { useTour } from '@/shared/lib/tour'

const { t } = useI18n()
const tour = useTour()

async function reset() {
  try {
    await tour.resetAll()
    toast.success(t('tour.resetDone'))
  } catch {
    toast.error(t('settings.saveFailed'))
  }
}
</script>

<template>
  <Card>
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="grid gap-0.5">
        <p class="text-sm font-medium">{{ t('tour.resetTitle') }}</p>
        <p class="text-xs text-muted-foreground">{{ t('tour.resetHint') }}</p>
      </div>
      <Button variant="outline" @click="reset">
        <GraduationCap class="size-4" />
        {{ t('tour.reset') }}
      </Button>
    </div>
  </Card>
</template>
