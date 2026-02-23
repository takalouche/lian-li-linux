<script setup lang="ts">
import { computed, ref } from "vue";
import type { FanCurve } from "../types";

const props = defineProps<{
  curve: FanCurve;
  index: number;
}>();

const emit = defineEmits<{
  update: [index: number, curve: FanCurve];
  remove: [index: number];
}>();

// SVG dimensions
const width = 400;
const height = 200;
const padding = { top: 10, right: 10, bottom: 30, left: 40 };
const plotW = width - padding.left - padding.right;
const plotH = height - padding.top - padding.bottom;

// Axis ranges
const tempMin = 20;
const tempMax = 100;
const speedMin = 0;
const speedMax = 100;

// Dragging stores the ORIGINAL array index, not the sorted index
const dragging = ref<number | null>(null);
let wasDragging = false;

// Sorted points with original index preserved for stable drag
const sortedPoints = computed(() =>
  props.curve.curve
    .map((point, orig) => ({ orig, point }))
    .sort((a, b) => a.point[0] - b.point[0])
);

function toSvg(temp: number, speed: number): { x: number; y: number } {
  const x = padding.left + ((temp - tempMin) / (tempMax - tempMin)) * plotW;
  const y = padding.top + (1 - (speed - speedMin) / (speedMax - speedMin)) * plotH;
  return { x, y };
}

function fromSvg(clientX: number, clientY: number, svg: SVGSVGElement): [number, number] {
  const ctm = svg.getScreenCTM();
  if (!ctm) return [tempMin, speedMin];
  const pt = new DOMPoint(clientX, clientY).matrixTransform(ctm.inverse());
  const temp = tempMin + ((pt.x - padding.left) / plotW) * (tempMax - tempMin);
  const speed = speedMax - ((pt.y - padding.top) / plotH) * (speedMax - speedMin);
  return [
    Math.round(Math.max(tempMin, Math.min(tempMax, temp))),
    Math.round(Math.max(speedMin, Math.min(speedMax, speed))),
  ];
}

const pathD = computed(() => {
  if (sortedPoints.value.length === 0) return "";
  return sortedPoints.value
    .map((p, i) => {
      const { x, y } = toSvg(p.point[0], p.point[1]);
      return `${i === 0 ? "M" : "L"} ${x} ${y}`;
    })
    .join(" ");
});

// Dashed clamp lines: extend from first point left to axis, last point right to axis
const clampLeft = computed(() => {
  if (sortedPoints.value.length === 0) return null;
  const first = sortedPoints.value[0].point;
  if (first[0] <= tempMin) return null;
  return { from: toSvg(tempMin, first[1]), to: toSvg(first[0], first[1]) };
});

const clampRight = computed(() => {
  if (sortedPoints.value.length === 0) return null;
  const last = sortedPoints.value[sortedPoints.value.length - 1].point;
  if (last[0] >= tempMax) return null;
  return { from: toSvg(last[0], last[1]), to: toSvg(tempMax, last[1]) };
});

// Grid lines
const tempTicks = [20, 30, 40, 50, 60, 70, 80, 90, 100];
const speedTicks = [0, 20, 40, 60, 80, 100];

function onMouseDown(origIdx: number) {
  dragging.value = origIdx;
  wasDragging = true;
}

function onMouseMove(e: MouseEvent) {
  if (dragging.value == null) return;
  const svg = (e.target as Element).closest("svg") as SVGSVGElement;
  if (!svg) return;
  const [temp, speed] = fromSvg(e.clientX, e.clientY, svg);
  const newCurve = [...props.curve.curve];
  newCurve[dragging.value] = [temp, speed];
  emit("update", props.index, { ...props.curve, curve: newCurve });
}

function onMouseUp() {
  dragging.value = null;
}

function onSvgClick(e: MouseEvent) {
  if (wasDragging) {
    wasDragging = false;
    return;
  }
  const svg = (e.target as Element).closest("svg") as SVGSVGElement;
  if (!svg) return;
  const [temp, speed] = fromSvg(e.clientX, e.clientY, svg);
  const newCurve = [...props.curve.curve, [temp, speed] as [number, number]];
  emit("update", props.index, { ...props.curve, curve: newCurve });
}

function removePoint(origIdx: number) {
  const newCurve = props.curve.curve.filter((_, i) => i !== origIdx);
  emit("update", props.index, { ...props.curve, curve: newCurve });
}
</script>

<template>
  <div class="rounded-xl border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5 transition-colors hover:border-gray-300 dark:hover:border-gray-600 hover:shadow-sm">
    <div class="flex items-center justify-between mb-3">
      <div class="flex items-center gap-3">
        <input
          :value="curve.name"
          @input="emit('update', index, { ...curve, name: ($event.target as HTMLInputElement).value })"
          class="font-semibold text-sm bg-transparent border-b border-transparent hover:border-gray-300 dark:hover:border-gray-600 focus:border-blue-500 outline-none px-0.5"
          placeholder="Curve name"
        />
      </div>
      <button
        @click="emit('remove', index)"
        class="text-xs text-red-500 hover:text-red-700 dark:hover:text-red-400"
      >
        Remove
      </button>
    </div>

    <div class="mb-3">
      <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Temperature Command</label>
      <input
        :value="curve.temp_command"
        @input="emit('update', index, { ...curve, temp_command: ($event.target as HTMLInputElement).value })"
        class="w-full px-2.5 py-1.5 text-sm font-mono rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-700"
        placeholder="cat /sys/class/thermal/thermal_zone0/temp | awk '{print $1/1000}'"
      />
    </div>

    <svg
      :width="width"
      :height="height"
      class="w-full bg-gray-50 dark:bg-gray-900 rounded-lg cursor-crosshair select-none"
      :viewBox="`0 0 ${width} ${height}`"
      @click="onSvgClick"
      @mousemove="onMouseMove"
      @mouseup="onMouseUp"
      @mouseleave="onMouseUp"
    >
      <!-- Grid -->
      <line
        v-for="t in tempTicks"
        :key="'gt-' + t"
        :x1="toSvg(t, 0).x"
        :y1="padding.top"
        :x2="toSvg(t, 0).x"
        :y2="padding.top + plotH"
        stroke="currentColor"
        class="text-gray-200 dark:text-gray-700"
        stroke-width="0.5"
      />
      <line
        v-for="s in speedTicks"
        :key="'gs-' + s"
        :x1="padding.left"
        :y1="toSvg(0, s).y"
        :x2="padding.left + plotW"
        :y2="toSvg(0, s).y"
        stroke="currentColor"
        class="text-gray-200 dark:text-gray-700"
        stroke-width="0.5"
      />

      <!-- Axis labels -->
      <text
        v-for="t in tempTicks"
        :key="'lt-' + t"
        :x="toSvg(t, 0).x"
        :y="height - 5"
        text-anchor="middle"
        class="text-[9px] fill-gray-400"
      >
        {{ t }}°
      </text>
      <text
        v-for="s in speedTicks"
        :key="'ls-' + s"
        :x="padding.left - 5"
        :y="toSvg(0, s).y + 3"
        text-anchor="end"
        class="text-[9px] fill-gray-400"
      >
        {{ s }}%
      </text>

      <!-- Clamp lines (dashed) — shows clamped speed outside curve range -->
      <line
        v-if="clampLeft"
        :x1="clampLeft.from.x" :y1="clampLeft.from.y"
        :x2="clampLeft.to.x" :y2="clampLeft.to.y"
        stroke="#3b82f6" stroke-width="1.5" stroke-dasharray="4 3" opacity="0.5"
      />
      <line
        v-if="clampRight"
        :x1="clampRight.from.x" :y1="clampRight.from.y"
        :x2="clampRight.to.x" :y2="clampRight.to.y"
        stroke="#3b82f6" stroke-width="1.5" stroke-dasharray="4 3" opacity="0.5"
      />

      <!-- Curve line -->
      <path :d="pathD" fill="none" stroke="#3b82f6" stroke-width="2" />

      <!-- Points (rendered in sorted order for visual, but mousedown uses original index) -->
      <circle
        v-for="sp in sortedPoints"
        :key="sp.orig"
        :cx="toSvg(sp.point[0], sp.point[1]).x"
        :cy="toSvg(sp.point[0], sp.point[1]).y"
        r="5"
        fill="#3b82f6"
        stroke="white"
        stroke-width="2"
        class="cursor-grab"
        :class="{ 'cursor-grabbing': dragging === sp.orig }"
        @mousedown.stop="onMouseDown(sp.orig)"
        @click.stop
        @dblclick.stop="removePoint(sp.orig)"
      />
    </svg>

    <p class="text-xs text-gray-400 mt-1">
      Click to add points. Drag to move. Double-click to remove.
    </p>
  </div>
</template>
