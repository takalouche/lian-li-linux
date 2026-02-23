<script setup lang="ts">
import { computed, watch } from "vue";
import { useConfigStore } from "../stores/config";
import { useDeviceStore } from "../stores/devices";
import FanCurveEditor from "../components/FanCurveEditor.vue";
import PageHeader from "../components/PageHeader.vue";
import type { FanCurve, FanGroup, FanSpeed, FanConfig } from "../types";

const configStore = useConfigStore();
const deviceStore = useDeviceStore();

const curveNames = computed(() => configStore.fanCurves.map((c) => c.name));

// Devices that have fan capability — these are the assignable units
const fanDevices = computed(() =>
  deviceStore.devices.filter((d) => d.has_fan && (d.fan_count ?? 0) > 0)
);

// Get or create fan config
const fanConfig = computed<FanConfig>(() =>
  configStore.fanConfig ?? { speeds: [], update_interval_ms: 1000 }
);

// Map device_id → FanGroup from config
function groupFor(deviceId: string): FanGroup | undefined {
  return fanConfig.value.speeds.find((g) => g.device_id === deviceId);
}

// Get the speed for a device (per-port) or a specific fan (per-fan)
function speedFor(deviceId: string, fanIdx: number = 0): FanSpeed {
  const group = groupFor(deviceId);
  if (!group) return 0;
  return group.speeds[fanIdx] ?? 0;
}

// Determine what the dropdown should show
function dropdownValue(speed: FanSpeed): string {
  if (typeof speed === "number" && speed > 0) return "__constant__";
  if (speed === "__mb_sync__") return "__mb_sync__";
  return String(speed);
}

function isConstant(speed: FanSpeed): boolean {
  return typeof speed === "number" && speed > 0;
}

function pwmPercent(speed: FanSpeed): number {
  if (typeof speed === "number") return Math.round((speed / 255) * 100);
  return 50;
}

function updateSpeed(deviceId: string, fanIdx: number, value: string) {
  const fc = { ...fanConfig.value, speeds: fanConfig.value.speeds.map((g) => ({ ...g, speeds: [...g.speeds] as [FanSpeed, FanSpeed, FanSpeed, FanSpeed] })) };
  let group = fc.speeds.find((g) => g.device_id === deviceId);
  if (!group) {
    group = { device_id: deviceId, speeds: [0, 0, 0, 0] };
    fc.speeds.push(group);
  }

  let speed: FanSpeed;
  if (value === "__constant__") {
    speed = 128; // default 50% PWM (128/255)
  } else {
    const num = parseInt(value);
    speed = isNaN(num) ? value : num;
  }
  group.speeds[fanIdx] = speed;
  configStore.updateFanConfig(fc);
}

function updatePwm(deviceId: string, fanIdx: number, percent: number) {
  const fc = { ...fanConfig.value, speeds: fanConfig.value.speeds.map((g) => ({ ...g, speeds: [...g.speeds] as [FanSpeed, FanSpeed, FanSpeed, FanSpeed] })) };
  let group = fc.speeds.find((g) => g.device_id === deviceId);
  if (!group) {
    group = { device_id: deviceId, speeds: [0, 0, 0, 0] };
    fc.speeds.push(group);
  }
  group.speeds[fanIdx] = Math.round((percent / 100) * 255);
  configStore.updateFanConfig(fc);
}

// Auto-sync: ensure config has groups for all detected devices
watch(fanDevices, (devices) => {
  if (!configStore.config) return;
  const fc = fanConfig.value;
  let changed = false;
  const newSpeeds = [...fc.speeds];

  for (const dev of devices) {
    if (!newSpeeds.some((g) => g.device_id === dev.device_id)) {
      newSpeeds.push({ device_id: dev.device_id, speeds: [0, 0, 0, 0] });
      changed = true;
    }
  }

  // Remove groups for devices no longer present
  const deviceIds = new Set(devices.map((d) => d.device_id));
  const filtered = newSpeeds.filter((g) => !g.device_id || deviceIds.has(g.device_id));
  if (filtered.length !== newSpeeds.length) changed = true;

  if (changed) {
    configStore.updateFanConfig({ ...fc, speeds: filtered });
  }
}, { immediate: true });

function handleCurveUpdate(index: number, curve: FanCurve) {
  const curves = [...configStore.fanCurves];
  curves[index] = curve;
  configStore.updateFanCurves(curves);
}

function handleCurveRemove(index: number) {
  const curves = configStore.fanCurves.filter((_, i) => i !== index);
  configStore.updateFanCurves(curves);
}

function addCurve() {
  const curves = [
    ...configStore.fanCurves,
    {
      name: `curve-${configStore.fanCurves.length + 1}`,
      temp_command: "cat /sys/class/thermal/thermal_zone0/temp | awk '{print $1/1000}'",
      curve: [
        [30, 30],
        [50, 50],
        [70, 80],
        [85, 100],
      ] as [number, number][],
    },
  ];
  configStore.updateFanCurves(curves);
}
</script>

<template>
  <div>
    <PageHeader title="Fan Configuration">
      <template #actions>
        <button
          @click="addCurve"
          class="px-3 py-1.5 text-sm rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors"
        >
          + Add Curve
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

    <div v-else>
      <!-- Fan curves -->
      <div v-if="configStore.fanCurves.length === 0" class="text-center py-8 mb-6">
        <p class="text-gray-500 dark:text-gray-400 text-sm">No fan curves configured.</p>
        <button
          @click="addCurve"
          class="mt-3 px-4 py-2 text-sm rounded-lg bg-blue-500 text-white hover:bg-blue-600"
        >
          Add Fan Curve
        </button>
      </div>

      <div v-else class="space-y-4 mb-8">
        <FanCurveEditor
          v-for="(curve, index) in configStore.fanCurves"
          :key="index"
          :curve="curve"
          :index="index"
          @update="handleCurveUpdate"
          @remove="handleCurveRemove"
        />
      </div>

      <!-- Fan speed assignment — auto-populated from detected devices -->
      <div class="mt-6">
        <h3 class="text-sm font-semibold mb-3">Fan Speed Assignment</h3>

        <div v-if="fanDevices.length === 0" class="rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">No fan devices detected.</p>
        </div>

        <div v-else class="space-y-3">
          <div
            v-for="dev in fanDevices"
            :key="dev.device_id"
            class="rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-4"
          >
            <div class="flex items-center justify-between mb-3">
              <div>
                <span class="text-sm font-semibold">{{ dev.name }}</span>
                <span class="ml-2 text-xs text-gray-400">{{ dev.fan_count }} fan(s)</span>
              </div>
              <span class="text-xs text-gray-400 font-mono">{{ dev.device_id }}</span>
            </div>

            <!-- Per-fan control: show individual fan dropdowns -->
            <div v-if="dev.per_fan_control" class="grid gap-2" :style="{ gridTemplateColumns: `repeat(${Math.min(dev.fan_count ?? 1, 4)}, 1fr)` }">
              <div v-for="fIdx in (dev.fan_count ?? 1)" :key="fIdx">
                <label class="block text-xs text-gray-400 dark:text-gray-500 mb-1">Fan {{ fIdx }}</label>
                <select
                  :value="dropdownValue(speedFor(dev.device_id, fIdx - 1))"
                  @change="updateSpeed(dev.device_id, fIdx - 1, ($event.target as HTMLSelectElement).value)"
                  class="w-full px-1.5 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
                >
                  <option value="0">Off</option>
                  <option v-for="name in curveNames" :key="name" :value="name">{{ name }}</option>
                  <option value="__constant__">Constant PWM</option>
                  <option v-if="dev.mb_sync_support" value="__mb_sync__">MB Sync</option>
                </select>
                <!-- PWM slider for constant mode -->
                <div v-if="isConstant(speedFor(dev.device_id, fIdx - 1))" class="flex items-center gap-2 mt-1">
                  <input
                    type="range"
                    min="0"
                    max="100"
                    :value="pwmPercent(speedFor(dev.device_id, fIdx - 1))"
                    @input="updatePwm(dev.device_id, fIdx - 1, parseInt(($event.target as HTMLInputElement).value))"
                    class="flex-1 h-1.5 accent-blue-500"
                  />
                  <span class="text-xs text-gray-400 w-8 text-right">{{ pwmPercent(speedFor(dev.device_id, fIdx - 1)) }}%</span>
                </div>
              </div>
            </div>

            <!-- Per-port control: single dropdown for all fans -->
            <div v-else>
              <label class="block text-xs text-gray-400 dark:text-gray-500 mb-1">All fans on this port</label>
              <select
                :value="dropdownValue(speedFor(dev.device_id, 0))"
                @change="updateSpeed(dev.device_id, 0, ($event.target as HTMLSelectElement).value)"
                class="w-full px-1.5 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
              >
                <option value="0">Off</option>
                <option v-for="name in curveNames" :key="name" :value="name">{{ name }}</option>
                <option value="__constant__">Constant PWM</option>
                <option v-if="dev.mb_sync_support" value="__mb_sync__">MB Sync</option>
              </select>
              <!-- PWM slider for constant mode -->
              <div v-if="isConstant(speedFor(dev.device_id, 0))" class="flex items-center gap-2 mt-1">
                <input
                  type="range"
                  min="0"
                  max="100"
                  :value="pwmPercent(speedFor(dev.device_id, 0))"
                  @input="updatePwm(dev.device_id, 0, parseInt(($event.target as HTMLInputElement).value))"
                  class="flex-1 h-1.5 accent-blue-500"
                />
                <span class="text-xs text-gray-400 w-8 text-right">{{ pwmPercent(speedFor(dev.device_id, 0)) }}%</span>
              </div>
            </div>
          </div>
        </div>

        <div class="mt-3" v-if="configStore.fanConfig">
          <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
            Update Interval (ms)
          </label>
          <input
            type="number"
            :value="configStore.fanConfig.update_interval_ms"
            @input="
              configStore.updateFanConfig({
                ...configStore.fanConfig!,
                update_interval_ms: parseInt(($event.target as HTMLInputElement).value) || 1000,
              })
            "
            class="w-40 px-2.5 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
            min="100"
            step="100"
          />
        </div>
      </div>
    </div>
  </div>
</template>
