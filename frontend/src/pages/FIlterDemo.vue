<template>
  <div class="demo-page">
    <h2>FilterBar Demo</h2>

    <FilterBar @filters-changed="onFiltersChanged"/>

    <section class="debug">
      <h3>Last emitted filters</h3>
      <pre>{{ JSON.stringify(currentFilters, null, 2) }}</pre>

      <h3>Current route query</h3>
      <pre>{{ JSON.stringify(routeQuery, null, 2) }}</pre>
    </section>
  </div>
</template>

<script lang="ts" setup>
import {ref, computed} from 'vue'
import {useRoute} from 'vue-router'
import FilterBar from '@/components/FilterBar.vue'

// const currentFilters = ref<Record<string, any>>({})
const currentFilters = ref({} as Record<string, any>)

const route = useRoute()
const routeQuery = computed(() => route.query)

function onFiltersChanged(payload: Record<string, any>) {
  currentFilters.value = payload
}
</script>

<style scoped>
.demo-page {
  padding: 1rem;
}

.debug {
  margin-top: 1rem;
  background: #f8f9fb;
  padding: 0.75rem;
  border-radius: 6px;
}

pre {
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 0.9rem;
}
</style>
