import { createRouter, createWebHistory } from 'vue-router'

export default createRouter({
  history: createWebHistory(import.meta.env.PROD ? '/architectui-vue-free/' : '/'),
  scrollBehavior () {
    return { top: 0, behavior: 'smooth' }
  },
  routes: [
    {
      path: '/',
      redirect: '/pr/top-files'
    },
    {
      path: '/filters',
      name: 'FilterDemo',
      component: () => import('../pages/FIlterDemo.vue')
    },
    {
      path: '/reviewers',
      name: 'Reviewers',
      component: () => import('../pages/ReviewersList.vue')
    },

    // PR Analytics
    {
      path: '/pr/top-files',
      name: 'PrTopFiles',
      component: () => import('../pages/pr/TopFilesPage.vue')
    },
    {
      path: '/pr/status',
      name: 'PrStatus',
      component: () => import('../pages/pr/PrStatusPage.vue')
    },
    {
      path: '/pr/status-over-time',
      name: 'PrStateOverTime',
      component: () => import('../pages/pr/PrStateOverTimePage.vue')
    },
    {
      path: '/pr/waiting',
      name: 'PrWaiting',
      component: () => import('../pages/pr/PrWaitingPage.vue')
    },
    {
      path: '/pr/team-files',
      name: 'PrTeamFiles',
      component: () => import('../pages/pr/TeamFilesPage.vue')
    },
    {
      path: '/pr/history',
      name: 'PrHistory',
      component: () => import('../pages/pr/PrHistoryPage.vue')
    },

    // Issues
    {
      path: '/issues/history',
      name: 'IssueHistory',
      component: () => import('../pages/issues/IssueHistoryPage.vue')
    }
  ]
})
