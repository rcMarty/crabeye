<script lang="ts">
import { defineComponent, computed, ref } from 'vue'
import { useReviewersStore } from '@/stores/reviewers'
import { BBadge, BSpinner } from 'bootstrap-vue-next'
import FilterBar from '@/components/FilterBar.vue'

export default defineComponent({
  name: 'ReviewersList',
  components: { BBadge, BSpinner, FilterBar },
  setup() {
    const store = useReviewersStore()
    const searchTerm = ref('')
    const items = computed(() => store.items)
    const isLoading = computed(() => store.isLoading)
    const page = computed(() => store.page)
    const perPage = computed(() => store.perPage)
    const totalCount = computed(() => store.totalCount)

    // called when the FilterBar live-updates filters (keeps local search text)
    function onFiltersChanged(payload: { q: string; status: string; tags: string[] }) {
      searchTerm.value = payload.q ?? ''
    }

    // called when the FilterBar form is submitted — fetch new data with filters
    async function onFiltersSubmitted(payload: {
      q?: string
      status?: string
      tags?: string[]
      file?: string
      from_date?: string | null
      last_n_days?: number | null
    }) {
      searchTerm.value = payload.q ?? ''
      // reset to first page when applying new filters
      await store.fetchReviewers({
        file: payload.file,
        from_date: payload.from_date ?? null,
        last_n_days: payload.last_n_days ?? null,
        page: 1,
        per_page: perPage.value
      })
    }

    function prev() {
      if (page.value > 1) store.setPage(page.value - 1)
    }

    function next() {
      const maxPage = Math.ceil(totalCount.value / perPage.value) || 1
      if (page.value < maxPage) store.setPage(page.value + 1)
    }

    const filteredItems = computed(() => {
      const q = searchTerm.value.trim().toLowerCase()
      if (!q) return items.value
      return items.value.filter(r => (r.github_name || '').toLowerCase().includes(q))
    })

    return {
      searchTerm,
      filteredItems,
      isLoading,
      page,
      perPage,
      totalCount,
      prev,
      next,
      onFiltersChanged,
      onFiltersSubmitted
    }
  }
})
</script>

<template>
  <div>
    <filter-bar @filters-changed="onFiltersChanged" @filters-submitted="onFiltersSubmitted" />

    <div v-if="isLoading" class="mb-2">
      <b-spinner label="Loading..." small />
      Loading reviewers...
    </div>

    <div v-else>
      <div v-if="filteredItems.length === 0" class="text-muted">No reviewers found.</div>

      <div v-else class="mb-2">
        <b-badge
          v-for="r in filteredItems"
          :key="r.github_id"
          :href="`https://github.com/${r.github_name}`"
          bg="primary"
          class="me-1 mb-1"
          pill
          target="_blank"
        >
          {{ r.github_name }} : {{ r.github_id }}
        </b-badge>
      </div>

      <div class="d-flex align-items-center gap-2">
        <button :disabled="page <= 1" class="btn btn-sm btn-outline-secondary" @click="prev">Prev</button>
        <small class="text-muted">Page {{ page }}</small>
        <button class="btn btn-sm btn-outline-secondary" @click="next">Next</button>
        <small class="ms-3 text-muted">Total: {{ totalCount }}</small>
      </div>
    </div>
  </div>
</template>

<style scoped>
.me-1 {
  margin-right: 0.25rem;
}

.mb-1 {
  margin-bottom: 0.25rem;
}
</style>
