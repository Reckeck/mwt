<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'
import { Star } from 'lucide-vue-next'

import { useFavoriteCurrencies } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { cn } from '@/shared/lib/utils'

import type { FavoriteStarButtonProps } from './currency-favorite.types'

/** Mark a currency or coin as a favourite, or unmark it: it then leads every list of currencies. */
const props = withDefaults(defineProps<FavoriteStarButtonProps>(), { size: 'xs' })

const { t } = useI18n()
const favorites = useFavoriteCurrencies()

const on = computed(() => favorites.isFavorite(props.code))
const label = computed(() =>
  t(on.value ? 'currency.favoriteRemove' : 'currency.favoriteAdd', { code: props.code.toUpperCase() }),
)

const toggle = useMutation({
  mutationFn: () => favorites.toggle(props.code),
  // No success toast: the star itself shows the new state. A failure is still said.
  meta: { success: '', error: 'settings.saveFailed' },
})
</script>

<template>
  <Button
    variant="ghost"
    size="icon"
    :class="cn(props.size === 'xs' ? 'size-7' : 'size-8')"
    :aria-pressed="on"
    :aria-label="label"
    :title="label"
    :disabled="toggle.isPending.value"
    @click.stop="toggle.mutate()"
  >
    <Star :class="cn('size-4', on ? 'fill-warning text-warning' : 'text-muted-foreground')" />
  </Button>
</template>
