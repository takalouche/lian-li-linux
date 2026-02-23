<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useDeviceStore } from "../stores/devices";
import { useConfigStore } from "../stores/config";
import PageHeader from "../components/PageHeader.vue";

const deviceStore = useDeviceStore();
const configStore = useConfigStore();
const socketPath = ref("...");

onMounted(async () => {
  socketPath.value = await invoke<string>("get_socket_path");
});
</script>

<template>
  <div>
    <PageHeader title="Settings" />

    <div class="max-w-lg space-y-6">
      <!-- Daemon status -->
      <div class="rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5">
        <h3 class="font-semibold text-sm mb-3">Daemon Status</h3>
        <div class="space-y-2 text-sm">
          <div class="flex items-center justify-between">
            <span class="text-gray-500 dark:text-gray-400">Connection</span>
            <span
              class="flex items-center gap-1.5"
              :class="deviceStore.daemonConnected ? 'text-green-600 dark:text-green-400' : 'text-red-500'"
            >
              <span
                class="w-2 h-2 rounded-full"
                :class="deviceStore.daemonConnected ? 'bg-green-500' : 'bg-red-500'"
              />
              {{ deviceStore.daemonConnected ? "Connected" : "Disconnected" }}
            </span>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-gray-500 dark:text-gray-400">Socket</span>
            <span class="font-mono text-xs">{{ socketPath }}</span>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-gray-500 dark:text-gray-400">Streaming</span>
            <span>{{ deviceStore.telemetry.streaming_active ? "Active" : "Idle" }}</span>
          </div>
        </div>
      </div>

      <!-- Config settings -->
      <div class="rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5">
        <h3 class="font-semibold text-sm mb-3">Configuration</h3>
        <div class="space-y-3">
          <div>
            <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
              Default FPS
            </label>
            <input
              type="number"
              :value="configStore.config?.default_fps ?? 30"
              @input="configStore.setDefaultFps(parseFloat(($event.target as HTMLInputElement).value) || 30)"
              class="w-40 px-2.5 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
              min="1"
              max="60"
              step="1"
            />
          </div>

          <div class="flex items-center justify-between text-sm">
            <span class="text-gray-500 dark:text-gray-400">LCD entries</span>
            <span>{{ configStore.lcds.length }}</span>
          </div>
          <div class="flex items-center justify-between text-sm">
            <span class="text-gray-500 dark:text-gray-400">Fan curves</span>
            <span>{{ configStore.fanCurves.length }}</span>
          </div>

          <button
            @click="configStore.save()"
            :disabled="!configStore.dirty || configStore.loading"
            class="w-full mt-2 px-4 py-2 text-sm rounded-lg font-medium transition-colors"
            :class="
              configStore.dirty
                ? 'bg-blue-500 text-white hover:bg-blue-600'
                : 'bg-gray-200 dark:bg-gray-700 text-gray-400 cursor-not-allowed'
            "
          >
            {{ configStore.loading ? "Saving..." : "Save Changes" }}
          </button>
        </div>
      </div>

      <!-- About -->
      <div class="rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5">
        <h3 class="font-semibold text-sm mb-3">About</h3>
        <div class="space-y-1 text-sm text-gray-500 dark:text-gray-400">
          <div>Lian Li Linux v0.1.0</div>
          <div>Linux replacement for L-Connect 3</div>
          <div class="text-xs mt-2">Fan speed control + LCD streaming for all Lian Li devices</div>
        </div>
      </div>
    </div>
  </div>
</template>
