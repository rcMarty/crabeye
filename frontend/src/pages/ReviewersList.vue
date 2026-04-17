<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert } from 'bootstrap-vue-next'
import {
  getReviewers,
  type Contributor,
  type PaginatedResponse,
  DEFAULT_REPOSITORY
} from '@/services/prApi'

const route = useRoute()
const router = useRouter()

// Filter state
const repository = ref(DEFAULT_REPOSITORY)
const filePath = ref('')
const anchorDate = ref('')
const lastNDays = ref<number | null>(30)

// Data
const items = ref<Contributor[]>([])
const totalCount = ref(0)
const page = ref(1)
const perPage = ref(25)
const loading = ref(false)
const error = ref<string | null>(null)
const searchTerm = ref('')

// Init from URL
onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (route.query.file) filePath.value = String(route.query.file)
  if (route.query.anchor_date) anchorDate.value = String(route.query.anchor_date)
  if (route.query.last_n_days) lastNDays.value = Number(route.query.last_n_days)
  if (route.query.page) page.value = Number(route.query.page)
  if (route.query.per_page) perPage.value = Number(route.query.per_page)

  if (filePath.value) fetchData()
})

// Sync URL
watch([repository, filePath, anchorDate, lastNDays, page, perPage], () => {
  router.replace({
    query: {
      ...(repository.value ? { repository: repository.value } : {}),
      ...(filePath.value ? { file: filePath.value } : {}),
      ...(anchorDate.value ? { anchor_date: anchorDate.value } : {}),
      ...(lastNDays.value != null ? { last_n_days: String(lastNDays.value) } : {}),
      page: String(page.value),
      per_page: String(perPage.value)
    }
  })
}, { flush: 'post' })

const totalPages = computed(() => Math.max(1, Math.ceil(totalCount.value / perPage.value)))

// Local search within the current page results
const filteredItems = computed(() => {
  const q = searchTerm.value.trim().toLowerCase()
  if (!q) return items.value
  return items.value.filter(r =>
    (r.github_name || '').toLowerCase().includes(q) ||
    (r.name || '').toLowerCase().includes(q)
  )
})

async function fetchData() {
  if (!filePath.value) {
    error.value = 'Please enter a file path or prefix to search'
    return
  }

  loading.value = true
  error.value = null
  try {
    const resp: PaginatedResponse<Contributor> = await getReviewers({
      repository: repository.value,
      file: filePath.value,
      anchor_date: anchorDate.value || undefined,
      last_n_days: lastNDays.value ?? undefined,
      pagination: { page: page.value, per_page: perPage.value }
    })
    items.value = resp.items
    totalCount.value = resp.total_count
    if (resp.page) page.value = resp.page
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch reviewers'
    items.value = []
    totalCount.value = 0
  } finally {
    loading.value = false
  }
}

function goToPage(p: number) {
  page.value = p
  fetchData()
}

function onPerPageChange() {
  page.value = 1
  fetchData()
}

function submitSearch() {
  page.value = 1
  fetchData()
}
</script>

<template>
  <div class="page-wrapper">
    <h1 class="page-title mb-1"><i class="pe-7s-users me-2"></i>File Reviewers</h1>
    <p class="text-muted mb-4">
      Find contributors who have modified files matching a given path prefix within a time window.
    </p>

    <!-- Filters -->
    <b-card class="filter-card mb-4">
      <div class="row g-3 align-items-end">
        <div class="col-md-3">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" placeholder="owner/repo" />
        </div>
        <div class="col-md-3">
          <label class="form-label">File Path / Prefix <span class="text-danger">*</span></label>
          <b-form-input
            v-model="filePath"
            placeholder="e.g. compiler/rustc_ast/"
            @keyup.enter="submitSearch"
          />
        </div>
        <div class="col-md-2">
          <label class="form-label">Anchor Date</label>
          <b-form-input v-model="anchorDate" type="date" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Last N Days</label>
          <b-form-input v-model.number="lastNDays" type="number" min="1" placeholder="30" />
        </div>
        <div class="col-md-2">
          <b-button variant="primary" class="w-100" @click="submitSearch" :disabled="loading">
            <b-spinner v-if="loading" small class="me-1" />
            {{ loading ? 'Searching…' : 'Search' }}
          </b-button>
        </div>
      </div>
    </b-card>

    <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">{{ error }}</b-alert>

    <!-- Results -->
    <template v-if="items.length > 0">
      <b-card>
        <div class="d-flex justify-content-between align-items-center mb-3">
          <h6 class="mb-0">
            Contributors
            <span class="text-muted fw-normal ms-2 small">({{ totalCount }} total)</span>
          </h6>
          <div class="d-flex gap-2 align-items-center">
            <input
              v-model="searchTerm"
              type="text"
              class="form-control form-control-sm"
              placeholder="Filter on page…"
              style="width: 180px;"
            />
            <select v-model.number="perPage" class="form-select form-select-sm" style="width: 80px;" @change="onPerPageChange">
              <option :value="10">10</option>
              <option :value="25">25</option>
              <option :value="50">50</option>
              <option :value="100">100</option>
            </select>
          </div>
        </div>

        <div class="table-responsive">
          <table class="table table-hover reviewer-table mb-0">
            <thead>
              <tr>
                <th style="width: 50px;">#</th>
                <th>GitHub Username</th>
                <th>Name</th>
                <th style="width: 120px;">GitHub ID</th>
                <th style="width: 100px;">Profile</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(r, idx) in filteredItems" :key="r.github_id">
                <td class="text-muted">{{ (page - 1) * perPage + idx + 1 }}</td>
                <td>
                  <div class="d-flex align-items-center gap-2">
                    <img
                      :src="`https://avatars.githubusercontent.com/u/${r.github_id}?s=32`"
                      :alt="r.github_name"
                      class="reviewer-avatar"
                      width="28"
                      height="28"
                      loading="lazy"
                    />
                    <strong>{{ r.github_name }}</strong>
                  </div>
                </td>
                <td class="text-muted">{{ r.name || '—' }}</td>
                <td class="text-muted small">{{ r.github_id }}</td>
                <td>
                  <a
                    :href="`https://github.com/${r.github_name}`"
                    target="_blank"
                    class="btn btn-sm btn-outline-primary"
                  >
                    <i class="pe-7s-link me-1"></i>Profile
                  </a>
                </td>
              </tr>
              <tr v-if="filteredItems.length === 0">
                <td colspan="5" class="text-center text-muted py-3">
                  No matches for "{{ searchTerm }}"
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- Pagination -->
        <div v-if="totalPages > 1" class="d-flex justify-content-between align-items-center mt-3 pt-2 border-top">
          <small class="text-muted">
            Page {{ page }} of {{ totalPages }} · {{ totalCount }} contributors total
          </small>
          <div class="d-flex gap-1 align-items-center">
            <button class="btn btn-sm btn-outline-secondary" :disabled="page <= 1" @click="goToPage(page - 1)">‹ Prev</button>
            <template v-for="p in totalPages" :key="p">
              <button
                v-if="p === 1 || p === totalPages || (p >= page - 2 && p <= page + 2)"
                class="btn btn-sm"
                :class="p === page ? 'btn-primary' : 'btn-outline-secondary'"
                @click="goToPage(p)"
              >{{ p }}</button>
              <span v-else-if="p === page - 3 || p === page + 3" class="px-1 text-muted">…</span>
            </template>
            <button class="btn btn-sm btn-outline-secondary" :disabled="page >= totalPages" @click="goToPage(page + 1)">Next ›</button>
          </div>
        </div>
      </b-card>
    </template>

    <div v-else-if="!loading && !error" class="text-center py-5 text-muted">
      <i class="pe-7s-users" style="font-size: 3rem; opacity: 0.3;"></i>
      <p class="mt-2">Enter a file path prefix and click "Search" to find contributors.</p>
    </div>
  </div>
</template>

<style scoped>
.page-wrapper { padding: 2rem; max-width: 1400px; margin: 0 auto; }
.page-title { font-size: 1.6rem; font-weight: 700; }
.filter-card { background: #fff; border: 1px solid #e9ecef; }
.form-label { font-weight: 500; margin-bottom: 0.4rem; }

.reviewer-table td { vertical-align: middle; }
.reviewer-avatar {
  border-radius: 50%;
  border: 2px solid #e9ecef;
  flex-shrink: 0;
}
</style>
