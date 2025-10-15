// File: `src/stores/reviewers.ts`
import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface Reviewer {
  github_name: string
  github_id: number
}

interface ReviewersResponse {
  items: Reviewer[]
  total_count: number
  page?: number
  per_page?: number
}

export const useReviewersStore = defineStore('reviewers', () => {
  const items = ref<Reviewer[]>([])
  const totalCount = ref<number>(0)
  const page = ref<number>(1)
  const perPage = ref<number>(10)
  const isLoading = ref<boolean>(false)
  const error = ref<string | null>(null)

  async function fetchReviewers(
    params: {
      file?: string
      from_date?: string | null
      last_n_days?: number | null
      page?: number
      per_page?: number
    } = {}
  ) {
    // merge URL query params into the provided params but don't override explicit args
    const effectiveParams: {
      file?: string
      from_date?: string | null
      last_n_days?: number | null
      page?: number
      per_page?: number
    } = { ...params }

    if (typeof window !== 'undefined' && window.location && window.location.search) {
      const urlParams = new URLSearchParams(window.location.search)

      if (urlParams.has('file') && effectiveParams.file === undefined) {
        effectiveParams.file = urlParams.get('file') || undefined
      }

      if (urlParams.has('from_date') && effectiveParams.from_date === undefined) {
        effectiveParams.from_date = urlParams.get('from_date') ?? null
      }

      if (urlParams.has('last_n_days') && effectiveParams.last_n_days === undefined) {
        const v = urlParams.get('last_n_days')
        const n = v ? parseInt(v, 10) : NaN
        effectiveParams.last_n_days = Number.isNaN(n) ? null : n
      }

      if (urlParams.has('page') && effectiveParams.page === undefined) {
        const v = parseInt(urlParams.get('page') || '', 10)
        if (!Number.isNaN(v)) effectiveParams.page = v
      }

      if (urlParams.has('per_page') && effectiveParams.per_page === undefined) {
        const v = parseInt(urlParams.get('per_page') || '', 10)
        if (!Number.isNaN(v)) effectiveParams.per_page = v
      }
    }

    page.value = effectiveParams.page ?? 1
    perPage.value = effectiveParams.per_page ?? 10
    isLoading.value = true
    error.value = null

    const qs = new URLSearchParams()
    if (effectiveParams.file) qs.set('file', effectiveParams.file)
    if (effectiveParams.from_date) qs.set('from_date', effectiveParams.from_date)
    if (effectiveParams.last_n_days != null) qs.set('last_n_days', String(effectiveParams.last_n_days))
    qs.set('page', String(page.value))
    qs.set('per_page', String(perPage.value))

    try {
      // Use Vite env var (import.meta.env) with fallback to relative path
      const base = (import.meta.env.VITE_BACKEND_BASE_URL as string) ?? ''
      const baseNoSlash = base.replace(/\/$/, '')
      const url = baseNoSlash ? `${baseNoSlash}/api/reviewers?${qs.toString()}` : `/api/reviewers?${qs.toString()}`

      console.log(`Fetching reviewers from: ${url}`)

      const res = await fetch(url)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = (await res.json()) as ReviewersResponse
      items.value = data.items ?? []
      totalCount.value = data.total_count ?? 0
      // keep page/per_page if backend returns them
      if (data.page) page.value = data.page
      if (data.per_page) perPage.value = data.per_page
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
      items.value = []
      totalCount.value = 0
    } finally {
      isLoading.value = false
    }
  }

  function setPage(p: number) {
    return fetchReviewers({ page: p, per_page: perPage.value })
  }

  return {
    items,
    totalCount,
    page,
    perPage,
    isLoading,
    error,
    fetchReviewers,
    setPage
  }
})
