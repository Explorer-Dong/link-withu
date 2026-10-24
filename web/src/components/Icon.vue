<script setup>
import {
  ArrowRight,
  ArrowUpRight,
  ChartNoAxesCombined,
  CalendarDays,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Clock3,
  Copy,
  Download,
  Eye,
  EyeOff,
  House,
  Link,
  Link2,
  LockKeyhole,
  LogOut,
  Palette,
  Pencil,
  Plus,
  Search,
  Settings,
  Tag,
  Trash2,
  X,
} from "@lucide/vue";

defineProps({ name: String, size: { type: [Number, String], default: 20 } });
const icons = {
  link: Link,
  links: Link2,
  home: House,
  chart: ChartNoAxesCombined,
  calendar: CalendarDays,
  settings: Settings,
  logout: LogOut,
  plus: Plus,
  search: Search,
  arrow: ArrowUpRight,
  "arrow-right": ArrowRight,
  copy: Copy,
  edit: Pencil,
  trash: Trash2,
  download: Download,
  tag: Tag,
  palette: Palette,
  lock: LockKeyhole,
  close: X,
  clock: Clock3,
  eye: Eye,
  "eye-off": EyeOff,
  "chevron-down": ChevronDown,
  "chevron-up": ChevronUp,
  "chevron-right": ChevronRight,
};
</script>
<template>
  <component
    :is="icons[name] || Link"
    :size="size"
    :stroke-width="1.75"
    aria-hidden="true"
  />
</template>
