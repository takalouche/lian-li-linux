// TypeScript types mirroring Rust structs in lianli-shared.
// Must match serde serialization exactly.

export type DeviceFamily =
  | "Ene6k77"
  | "TlFan"
  | "TlLcd"
  | "Galahad2Trinity"
  | "HydroShiftLcd"
  | "Galahad2Lcd"
  | "WirelessTx"
  | "WirelessRx"
  | "Slv3Lcd"
  | "Slv3Led"
  | "Tlv2Lcd"
  | "Tlv2Led"
  | "SlInf"
  | "Clv1"
  | "HydroShift2Lcd"
  | "Lancool207"
  | "UniversalScreen"
  | "DisplaySwitcher";

export interface DeviceInfo {
  device_id: string;
  family: DeviceFamily;
  name: string;
  serial: string | null;
  has_lcd: boolean;
  has_fan: boolean;
  has_pump: boolean;
  fan_count: number | null;
  per_fan_control: boolean | null;
  mb_sync_support: boolean;
  screen_width: number | null;
  screen_height: number | null;
}

export type MediaType = "image" | "video" | "color" | "gif" | "sensor";

export interface SensorRange {
  max: number | null;
  color: [number, number, number];
}

export interface SensorSourceConfig {
  type: "constant" | "command";
  value?: number; // for constant
  cmd?: string; // for command
}

export interface SensorDescriptor {
  label: string;
  unit: string;
  source: SensorSourceConfig;
  text_color: [number, number, number];
  background_color: [number, number, number];
  gauge_background_color: [number, number, number];
  gauge_ranges: SensorRange[];
  update_interval_ms: number;
  gauge_start_angle: number;
  gauge_sweep_angle: number;
  gauge_outer_radius: number;
  gauge_thickness: number;
  bar_corner_radius: number;
  value_font_size: number;
  unit_font_size: number;
  label_font_size: number;
  font_path: string | null;
  decimal_places: number;
  value_offset: number;
  unit_offset: number;
  label_offset: number;
}

export interface LcdConfig {
  index?: number;
  serial?: string;
  type: MediaType; // serde(rename = "type")
  path?: string;
  fps?: number;
  rgb?: [number, number, number];
  orientation: number;
  sensor?: SensorDescriptor;
}

export type FanSpeed = number | string; // number = constant PWM, string = curve name

export interface FanCurve {
  name: string;
  temp_command: string;
  curve: [number, number][]; // [temp_celsius, speed_percent]
}

export interface FanGroup {
  device_id?: string;
  speeds: [FanSpeed, FanSpeed, FanSpeed, FanSpeed];
}

export interface FanConfig {
  speeds: FanGroup[];
  update_interval_ms: number;
}

export interface AppConfig {
  default_fps: number;
  lcds: LcdConfig[];
  fan_curves: FanCurve[];
  fans?: FanConfig;
}

export interface TelemetrySnapshot {
  fan_rpms: Record<string, number[]>;
  coolant_temps: Record<string, number>;
  streaming_active: boolean;
}

// Helper: human-readable family names
export const FAMILY_NAMES: Record<DeviceFamily, string> = {
  Ene6k77: "UNI FAN SL/AL",
  TlFan: "UNI FAN TL",
  TlLcd: "UNI FAN TL LCD",
  Galahad2Trinity: "Galahad II Trinity AIO",
  HydroShiftLcd: "HydroShift LCD AIO",
  Galahad2Lcd: "Galahad II LCD AIO",
  WirelessTx: "Wireless TX Dongle",
  WirelessRx: "Wireless RX Dongle",
  Slv3Lcd: "UNI FAN SL Wireless LCD",
  Slv3Led: "UNI FAN SL Wireless",
  Tlv2Lcd: "UNI FAN TL Wireless LCD",
  Tlv2Led: "UNI FAN TL Wireless",
  SlInf: "UNI FAN SL-INF Wireless",
  Clv1: "UNI FAN CL Wireless",
  HydroShift2Lcd: "HydroShift II LCD Circle",
  Lancool207: "Lancool 207 Digital",
  UniversalScreen: 'Universal Screen 8.8"',
  DisplaySwitcher: "Display Mode Switcher",
};
