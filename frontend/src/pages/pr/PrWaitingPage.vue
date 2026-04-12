<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert } from 'bootstrap-vue-next'
import DoughnutChartComponent from '@/components/Charts/DoughnutChartComponent.vue'
import { getWaitingForReview, PrEvent, getStatusType, DEFAULT_REPOSITORY } from '@/services/prApi'

const route = useRoute()
const router = useRouter()

const repository = ref<string>(DEFAULT_REPOSITORY)
const waitingPrs = ref<PrEvent[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (repository.value) fetch()
})

watch([repository], () => {
  router.replace({
    query: { ...(repository.value ? { repository: repository.value } : {}) }
  })
}, { flush: 'post' })

const chartData = computed(() => ({
  labels: ['Waiting for Review'],
  datasets: [{
    label: 'PRs',
    data: [waitingPrs.value.length],
    backgroundColor: ['rgba(255, 206, 86, 0.8)'],
    borderColor: ['rgba(255, 206, 86, 1)'],
    borderWidth: 1
  }]
}))

const chartOptions = {
  plugins: {
    legend: { display: false },
    title: { display: true, text: 'PRs Waiting for Review' }
  }
}

async function fetch() {
  loading.value = true
  error.value = null
  try {
    const response = await getWaitingForReview({ repository: repository.value })
    waitingPrs.value = response.items
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch waiting PRs'
    waitingPrs.value = []
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="page-wrapper">
    <h1 class="mb-4">PRs Waiting for Review</h1>

    <b-card class="mb-4">
      <div class="row g-3">
        <div class="col-md-8">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" type="text" placeholder="owner/repo" />
        </div>
      </div>
      <div class="mt-3">
        <b-button variant="primary" @click="fetch" :disabled="loading">
          <b-spinner v-if="loading" small class="me-1" />
          {{ loading ? 'Loading...' : 'Fetch Data' }}
        </b-button>
      </div>
    </b-card>

    <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">{{ error }}</b-alert>

    <b-card v-if="waitingPrs.length > 0">
      <div class="row">
        <div class="col-md-5">
          <doughnut-chart-component :data="chartData" :options="chartOptions" :height="300" />
        </div>
        <div class="col-md-7">
          <div class="waiting-count">
            <div class="display-4">{{ waitingPrs.length }}</div>
            <div class="text-muted">PRs waiting for review</div>
          </div>
          <div class="mt-3">
            <h6>PR Numbers:</h6>
            <div class="pr-list">
              <span
                v-for="(pr, idx) in waitingPrs"
                :key="idx"
                class="badge bg-warning text-dark me-1 mb-1"
              >
                #{{ pr.pr_number }} ({{ getStatusType(pr.state) }})
              </span>
            </div>
          </div>
        </div>
      </div>
    </b-card>
    <div v-else-if="!loading" class="text-muted text-center py-5">
      Click "Fetch Data" to see PRs waiting for review
    </div>
  </div>
</template>

<style scoped>
.page-wrapper { padding: 2rem; max-width: 1400px; margin: 0 auto; }
.form-label { font-weight: 500; margin-bottom: 0.5rem; }
.waiting-count { text-align: center; padding: 2rem; background: #f8f9fa; border-radius: 0.5rem; }
.pr-list { max-height: 200px; overflow-y: auto; padding: 0.5rem; background: #f8f9fa; border-radius: 0.25rem; }
</style>
