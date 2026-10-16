<script setup>
import { onMounted, onUnmounted, ref } from "vue";
import Icon from "./Icon.vue";
defineProps({ title: String });
const emit = defineEmits(["close"]);
const dialog = ref(null);
let previous;
onMounted(() => {
  previous = document.activeElement;
  dialog.value.showModal();
});
onUnmounted(() => {
  previous?.focus();
});
</script>
<template>
  <dialog
    ref="dialog"
    class="modal"
    aria-labelledby="modal-title"
    @cancel.prevent="emit('close')"
    @click="
      (e) => {
        if (e.target === dialog) emit('close');
      }
    "
  >
    <div class="modal-heading">
      <h2 id="modal-title">{{ title }}</h2>
      <button class="icon-btn" aria-label="关闭" @click="emit('close')">
        <Icon name="close" />
      </button>
    </div>
    <slot />
  </dialog>
</template>
