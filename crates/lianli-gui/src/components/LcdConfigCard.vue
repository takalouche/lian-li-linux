<script setup lang="ts">
import { computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useDeviceStore } from "../stores/devices";
import type { LcdConfig, MediaType, SensorDescriptor, SensorRange } from "../types";

const props = defineProps<{
  lcd: LcdConfig;
  index: number;
}>();

const emit = defineEmits<{
  update: [index: number, lcd: LcdConfig];
  remove: [index: number];
}>();

const deviceStore = useDeviceStore();

const lcdDevices = computed(() =>
  deviceStore.devices.filter((d) => d.has_lcd)
);

const mediaTypes: { value: MediaType; label: string }[] = [
  { value: "image", label: "Image" },
  { value: "video", label: "Video" },
  { value: "gif", label: "GIF" },
  { value: "color", label: "Solid Color" },
  { value: "sensor", label: "Sensor Gauge" },
];

const orientations = [0, 90, 180, 270];

function updateField<K extends keyof LcdConfig>(field: K, value: LcdConfig[K]) {
  emit("update", props.index, { ...props.lcd, [field]: value });
}

function selectDevice(serial: string) {
  emit("update", props.index, {
    ...props.lcd,
    serial: serial || undefined,
    index: undefined,
  });
}

async function pickFile() {
  try {
    const path = await invoke<string | null>("pick_media_file");
    if (path) {
      updateField("path", path);
    }
  } catch {
    // User cancelled
  }
}

function updateRgb(channelIdx: number, value: string) {
  const rgb: [number, number, number] = props.lcd.rgb ? [...props.lcd.rgb] : [0, 0, 0];
  rgb[channelIdx] = Math.max(0, Math.min(255, parseInt(value) || 0));
  updateField("rgb", rgb);
}

const defaultSensor: SensorDescriptor = {
  label: "CPU",
  unit: "°C",
  source: { type: "command", cmd: "sensors 2>/dev/null | awk '/Package/ {print $4}' | tr -d '+°C'" },
  text_color: [255, 255, 255],
  background_color: [0, 0, 0],
  gauge_background_color: [0, 0, 0],
  gauge_ranges: [
    { max: 50, color: [0, 200, 0] },
    { max: 80, color: [220, 140, 0] },
    { max: null, color: [220, 0, 0] },
  ],
  update_interval_ms: 1000,
  gauge_start_angle: 60,
  gauge_sweep_angle: 300,
  gauge_outer_radius: 180,
  gauge_thickness: 40,
  bar_corner_radius: 20,
  value_font_size: 180,
  unit_font_size: 40,
  label_font_size: 40,
  font_path: "/usr/share/fonts/TTF/DejaVuSans.ttf",
  decimal_places: 0,
  value_offset: -90,
  unit_offset: 80,
  label_offset: -140,
};

const sensor = computed(() => props.lcd.sensor ?? defaultSensor);

function updateSensor(partial: Partial<SensorDescriptor>) {
  updateField("sensor", { ...sensor.value, ...partial });
}

function updateSensorColor(field: "text_color" | "background_color" | "gauge_background_color", idx: number, value: string) {
  const rgb: [number, number, number] = [...sensor.value[field]];
  rgb[idx] = Math.max(0, Math.min(255, parseInt(value) || 0));
  updateSensor({ [field]: rgb });
}

function updateRange(rIdx: number, partial: Partial<SensorRange>) {
  const ranges = sensor.value.gauge_ranges.map((r, i) =>
    i === rIdx ? { ...r, ...partial } : r
  );
  updateSensor({ gauge_ranges: ranges });
}

function updateRangeColor(rIdx: number, cIdx: number, value: string) {
  const color: [number, number, number] = [...sensor.value.gauge_ranges[rIdx].color];
  color[cIdx] = Math.max(0, Math.min(255, parseInt(value) || 0));
  updateRange(rIdx, { color });
}

function addRange() {
  const ranges = [...sensor.value.gauge_ranges, { max: null, color: [200, 0, 0] as [number, number, number] }];
  updateSensor({ gauge_ranges: ranges });
}

function removeRange(idx: number) {
  const ranges = sensor.value.gauge_ranges.filter((_, i) => i !== idx);
  updateSensor({ gauge_ranges: ranges });
}

// Ensure sensor object exists when switching to sensor type
function onTypeChange(newType: MediaType) {
  const updates: Partial<LcdConfig> = { type: newType };
  if (newType === "sensor" && !props.lcd.sensor) {
    updates.sensor = { ...defaultSensor };
  }
  emit("update", props.index, { ...props.lcd, ...updates });
}
</script>

<template>
  <div class="rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5 transition-colors hover:border-gray-300 dark:hover:border-gray-600 hover:shadow-sm">
    <div class="flex items-center justify-between mb-4">
      <h3 class="font-semibold text-sm">LCD {{ index + 1 }}</h3>
      <button
        @click="emit('remove', index)"
        class="text-xs text-red-500 hover:text-red-700 dark:hover:text-red-400"
      >
        Remove
      </button>
    </div>

    <div class="space-y-4">
      <!-- Device selection -->
      <div>
        <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Device</label>
        <select
          :value="lcd.serial ?? ''"
          @change="selectDevice(($event.target as HTMLSelectElement).value)"
          class="w-full px-2.5 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
        >
          <option value="">Select a device...</option>
          <option
            v-for="dev in lcdDevices"
            :key="dev.device_id"
            :value="dev.serial ?? dev.device_id"
          >
            {{ dev.name }} — {{ dev.serial ?? dev.device_id }}
            <template v-if="dev.screen_width"> ({{ dev.screen_width }}x{{ dev.screen_height }})</template>
          </option>
        </select>
      </div>

      <!-- Media type -->
      <div>
        <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Media Type</label>
        <select
          :value="lcd.type"
          @change="onTypeChange(($event.target as HTMLSelectElement).value as MediaType)"
          class="w-full px-2.5 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
        >
          <option v-for="mt in mediaTypes" :key="mt.value" :value="mt.value">
            {{ mt.label }}
          </option>
        </select>
      </div>

      <!-- File path (for image/video/gif) -->
      <div v-if="lcd.type === 'image' || lcd.type === 'video' || lcd.type === 'gif'">
        <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">File Path</label>
        <div class="flex gap-2">
          <input
            type="text"
            :value="lcd.path ?? ''"
            @input="updateField('path', ($event.target as HTMLInputElement).value || undefined)"
            class="flex-1 px-2.5 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
            placeholder="/path/to/media"
          />
          <button
            @click="pickFile()"
            class="px-3 py-1.5 text-sm rounded-lg bg-blue-500 text-white hover:bg-blue-600"
          >
            Browse
          </button>
        </div>
      </div>

      <!-- Color (for solid color) -->
      <div v-if="lcd.type === 'color'" class="grid grid-cols-3 gap-2">
        <div v-for="(label, i) in ['R', 'G', 'B']" :key="label">
          <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">{{ label }}</label>
          <input
            type="number"
            :value="lcd.rgb?.[i] ?? 0"
            @input="updateRgb(i, ($event.target as HTMLInputElement).value)"
            class="w-full px-2.5 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
            min="0"
            max="255"
          />
        </div>
      </div>

      <!-- Sensor config -->
      <div v-if="lcd.type === 'sensor'" class="space-y-3 rounded-lg border border-gray-200 dark:border-gray-600 p-3">
        <h4 class="text-xs font-semibold text-gray-500 dark:text-gray-400">Sensor Configuration</h4>

        <div class="grid grid-cols-3 gap-2">
          <div>
            <label class="block text-xs text-gray-500 dark:text-gray-400 mb-0.5">Label</label>
            <input type="text" :value="sensor.label" @input="updateSensor({ label: ($event.target as HTMLInputElement).value })" class="w-full px-2 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" />
          </div>
          <div>
            <label class="block text-xs text-gray-500 dark:text-gray-400 mb-0.5">Unit</label>
            <input type="text" :value="sensor.unit" @input="updateSensor({ unit: ($event.target as HTMLInputElement).value })" class="w-full px-2 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" />
          </div>
          <div>
            <label class="block text-xs text-gray-500 dark:text-gray-400 mb-0.5">Decimals</label>
            <input type="number" :value="sensor.decimal_places" @input="updateSensor({ decimal_places: parseInt(($event.target as HTMLInputElement).value) || 0 })" class="w-full px-2 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" min="0" max="3" />
          </div>
        </div>

        <div>
          <label class="block text-xs text-gray-500 dark:text-gray-400 mb-0.5">Source Command</label>
          <input type="text" :value="sensor.source.cmd ?? ''" @input="updateSensor({ source: { type: 'command', cmd: ($event.target as HTMLInputElement).value } })" class="w-full px-2 py-1 text-xs font-mono rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" />
        </div>

        <div>
          <label class="block text-xs text-gray-500 dark:text-gray-400 mb-0.5">Font Path</label>
          <input type="text" :value="sensor.font_path ?? ''" @input="updateSensor({ font_path: ($event.target as HTMLInputElement).value || null })" class="w-full px-2 py-1 text-xs font-mono rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" placeholder="/usr/share/fonts/..." />
        </div>

        <div class="grid grid-cols-2 gap-2">
          <div>
            <label class="block text-xs text-gray-500 dark:text-gray-400 mb-0.5">Update Interval (ms)</label>
            <input type="number" :value="sensor.update_interval_ms" @input="updateSensor({ update_interval_ms: parseInt(($event.target as HTMLInputElement).value) || 1000 })" class="w-full px-2 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" min="100" step="100" />
          </div>
        </div>

        <!-- Font Sizes -->
        <div class="grid grid-cols-3 gap-2">
          <div v-for="(label, field) in {
            value_font_size: 'Value Font Size',
            unit_font_size: 'Unit Font Size',
            label_font_size: 'Label Font Size',
          } as const" :key="field">
            <label class="block text-xs text-gray-500 dark:text-gray-400 mb-0.5">{{ label }}</label>
            <input type="number" :value="sensor[field]" @input="updateSensor({ [field]: parseFloat(($event.target as HTMLInputElement).value) || 0 })" class="w-full px-2 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" />
          </div>
        </div>

        <!-- Colors -->
        <div v-for="(colorLabel, colorField) in { text_color: 'Text Color', background_color: 'Background Color', gauge_background_color: 'Gauge BG Color' } as const" :key="colorField" class="flex items-center gap-2">
          <span class="text-xs text-gray-500 dark:text-gray-400 w-24 shrink-0">{{ colorLabel }}</span>
          <div v-for="(ch, ci) in ['R','G','B']" :key="ch" class="flex items-center gap-1">
            <span class="text-xs text-gray-400">{{ ch }}</span>
            <input type="number" :value="sensor[colorField][ci]" @input="updateSensorColor(colorField, ci, ($event.target as HTMLInputElement).value)" class="w-14 px-1.5 py-0.5 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" min="0" max="255" />
          </div>
        </div>

        <!-- Gauge Ranges -->
        <div>
          <div class="flex items-center justify-between mb-2">
            <span class="text-xs font-medium text-gray-500 dark:text-gray-400">Gauge Ranges</span>
            <button @click="addRange" class="text-xs text-blue-500 hover:text-blue-600">+ Add</button>
          </div>
          <div class="grid grid-cols-[auto_1fr_1fr_1fr_1fr_auto] gap-x-2 gap-y-1 items-center text-xs">
            <!-- Header -->
            <span></span>
            <span class="text-gray-400 dark:text-gray-500">Max Value</span>
            <span class="text-gray-400 dark:text-gray-500">R</span>
            <span class="text-gray-400 dark:text-gray-500">G</span>
            <span class="text-gray-400 dark:text-gray-500">B</span>
            <span></span>
            <!-- Rows -->
            <template v-for="(range, rIdx) in sensor.gauge_ranges" :key="rIdx">
              <span class="text-gray-400 dark:text-gray-500">{{ rIdx + 1 }}</span>
              <input type="number" :value="range.max ?? ''" @input="updateRange(rIdx, { max: ($event.target as HTMLInputElement).value ? parseInt(($event.target as HTMLInputElement).value) : null })" class="w-full px-1.5 py-0.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" placeholder="∞" />
              <input type="number" :value="range.color[0]" @input="updateRangeColor(rIdx, 0, ($event.target as HTMLInputElement).value)" class="w-full px-1.5 py-0.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" min="0" max="255" />
              <input type="number" :value="range.color[1]" @input="updateRangeColor(rIdx, 1, ($event.target as HTMLInputElement).value)" class="w-full px-1.5 py-0.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" min="0" max="255" />
              <input type="number" :value="range.color[2]" @input="updateRangeColor(rIdx, 2, ($event.target as HTMLInputElement).value)" class="w-full px-1.5 py-0.5 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" min="0" max="255" />
              <button @click="removeRange(rIdx)" class="text-red-500 hover:text-red-700">x</button>
            </template>
          </div>
        </div>

        <!-- Gauge Geometry (advanced) -->
        <details class="text-xs">
          <summary class="cursor-pointer text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300">Advanced Gauge Geometry</summary>
          <div class="grid grid-cols-3 gap-2 mt-2">
            <div v-for="(label, field) in {
              gauge_start_angle: 'Start Angle',
              gauge_sweep_angle: 'Sweep Angle',
              gauge_outer_radius: 'Outer Radius',
              gauge_thickness: 'Thickness',
              bar_corner_radius: 'Corner Radius',
              value_offset: 'Value Y Offset',
              unit_offset: 'Unit Y Offset',
              label_offset: 'Label Y Offset',
            } as const" :key="field">
              <div>
                <label class="block text-xs text-gray-500 dark:text-gray-400 mb-0.5">{{ label }}</label>
                <input type="number" :value="sensor[field]" @input="updateSensor({ [field]: parseFloat(($event.target as HTMLInputElement).value) || 0 })" class="w-full px-1.5 py-0.5 text-xs rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700" />
              </div>
            </div>
          </div>
        </details>
      </div>

      <!-- FPS and Orientation -->
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">FPS</label>
          <input
            type="number"
            :value="lcd.fps"
            @input="updateField('fps', ($event.target as HTMLInputElement).valueAsNumber || undefined)"
            class="w-full px-2.5 py-1.5 text-sm rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
            min="1"
            max="60"
          />
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Orientation</label>
          <div class="flex gap-1">
            <button
              v-for="deg in orientations"
              :key="deg"
              @click="updateField('orientation', deg)"
              class="flex-1 px-2 py-1.5 text-xs rounded-lg border transition-colors"
              :class="
                lcd.orientation === deg
                  ? 'bg-blue-500 text-white border-blue-500'
                  : 'border-gray-300 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-700'
              "
            >
              {{ deg }}°
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
