<script setup lang="ts">
import { useConfigStore } from "../stores/config";
import LcdConfigCard from "../components/LcdConfigCard.vue";
import PageHeader from "../components/PageHeader.vue";
import type { LcdConfig } from "../types";

const configStore = useConfigStore();

function handleUpdate(index: number, lcd: LcdConfig) {
  configStore.updateLcd(index, lcd);
}

function handleRemove(index: number) {
  configStore.removeLcd(index);
}

function addLcd() {
  configStore.addLcd({
    type: "image",
    orientation: 0,
  });
}
</script>

<template>
  <div>
    <PageHeader title="LCD Configuration">
      <template #actions>
        <button
          @click="addLcd"
          class="px-3 py-1.5 text-sm rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
        >
          + Add LCD
        </button>
        <button
          @click="configStore.save()"
          :disabled="!configStore.dirty || configStore.loading"
          class="px-4 py-1.5 text-sm rounded-lg font-medium transition-colors"
          :class="
            configStore.dirty
              ? 'bg-blue-500 text-white hover:bg-blue-600'
              : 'bg-gray-200 dark:bg-gray-700 text-gray-400 cursor-not-allowed'
          "
        >
          {{ configStore.loading ? "Saving..." : "Save" }}
        </button>
      </template>
    </PageHeader>

    <div v-if="configStore.error" class="mb-4 text-sm text-red-500">
      {{ configStore.error }}
    </div>

    <div v-if="!configStore.config" class="text-sm text-gray-500">
      No config loaded. Is the daemon running?
    </div>

    <div v-else-if="configStore.lcds.length === 0" class="text-center py-12">
      <p class="text-gray-500 dark:text-gray-400 text-sm">No LCD entries configured.</p>
      <button
        @click="addLcd"
        class="mt-3 px-4 py-2 text-sm rounded-lg bg-blue-500 text-white hover:bg-blue-600"
      >
        Add LCD Device
      </button>
    </div>

    <div v-else class="space-y-4">
      <LcdConfigCard
        v-for="(lcd, index) in configStore.lcds"
        :key="index"
        :lcd="lcd"
        :index="index"
        @update="handleUpdate"
        @remove="handleRemove"
      />
    </div>
  </div>
</template>
