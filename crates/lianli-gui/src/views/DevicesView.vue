<script setup lang="ts">
import { computed } from "vue";
import { useDeviceStore } from "../stores/devices";
import DeviceCard from "../components/DeviceCard.vue";
import PageHeader from "../components/PageHeader.vue";

const deviceStore = useDeviceStore();

// Hide wireless TX/RX dongle (internal), show everything else
const visibleDevices = computed(() =>
  deviceStore.devices.filter(
    (d) => d.family !== "WirelessTx" && d.family !== "WirelessRx"
  )
);
</script>

<template>
  <div>
    <PageHeader title="Devices">
      <template #actions>
        <button
          @click="deviceStore.refreshDevices()"
          class="px-3 py-1.5 text-sm rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
        >
          Refresh
        </button>
      </template>
    </PageHeader>

    <div v-if="deviceStore.loading" class="text-sm text-gray-500">Loading devices...</div>

    <div v-else-if="deviceStore.error" class="text-sm text-red-500">
      {{ deviceStore.error }}
    </div>

    <div v-else-if="visibleDevices.length === 0" class="text-center py-12">
      <p class="text-gray-500 dark:text-gray-400 text-sm">No Lian Li devices detected.</p>
      <p class="text-gray-400 dark:text-gray-500 text-xs mt-2">
        Make sure udev rules are installed and devices are connected.
      </p>
    </div>

    <div v-else class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      <DeviceCard
        v-for="device in visibleDevices"
        :key="device.device_id"
        :device="device"
      />
    </div>

    <div
      v-if="deviceStore.telemetry.streaming_active"
      class="mt-4 text-xs text-green-600 dark:text-green-400 flex items-center gap-1.5"
    >
      <span class="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
      LCD streaming active
    </div>
  </div>
</template>
