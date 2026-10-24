<script setup>
defineProps({ title: String });
</script>
<template>
  <div class="page-heading">
    <div class="page-title">
      <img src="/favicon.svg?v=lw" alt="Link WithU" width="44" height="44" />
      <h1>{{ title }}</h1>
    </div>
  </div>
</template>
