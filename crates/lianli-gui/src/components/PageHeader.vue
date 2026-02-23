<script setup lang="ts">
defineProps<{
  title: string;
}>();
</script>

<template>
  <div
    class="sticky top-0 z-10 -mx-6 px-6 pt-6 pb-4 mb-4 bg-white/80 dark:bg-gray-900/80 backdrop-blur-sm border-b border-gray-200/50 dark:border-gray-700/50"
  >
    <div class="flex items-center justify-between min-h-[34px]">
      <h2 class="text-xl font-bold">{{ title }}</h2>
      <div v-if="$slots.actions" class="flex gap-2">
        <slot name="actions" />
      </div>
    </div>
  </div>
</template>
