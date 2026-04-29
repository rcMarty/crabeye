import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export const useSidebarStore = defineStore('sidebar', () => {
  // State
  const isCollapsed = ref(false)
  const isMobile = ref(false)
  const activeSubmenus = ref(new Set())
  const windowWidth = ref(0)

  // Menu structure
  const menuItems = ref([
    {
      id: 'graphs',
      type: 'header',
      title: 'Graphs'
    },
    {
      id: 'pull-requests',
      type: 'submenu',
      title: 'Pull Requests',
      icon: 'pe-7s-graph1',
      children: [
        {
          id: 'pr-top-files',
          title: 'Top Modified Files',
          route: '/pr/top-files'
        },
        {
          id: 'pr-status',
          title: 'PR Status Distribution',
          route: '/pr/status'
        },
        {
          id: 'pr-status-over-time',
          title: 'State Over Time',
          route: '/pr/status-over-time'
        },
        {
          id: 'pr-waiting',
          title: 'Waiting for Review',
          route: '/pr/waiting'
        },
        {
          id: 'pr-team-files',
          title: 'Team File Changes',
          route: '/pr/team-files'
        },
        {
          id: 'files-changed',
          title: 'Files Changed (Reviewers)',
          route: '/reviewers'
        },
        {
          id: 'pr-history',
          title: 'PR History',
          route: '/pr/history'
        }
      ]
    },
    {
      id: 'issues-header',
      type: 'header',
      title: 'Issues'
    },
    {
      id: 'issues',
      type: 'submenu',
      title: 'Issues',
      icon: 'pe-7s-ticket',
      children: [
        {
          id: 'issue-events',
          title: 'Issue Events',
          route: '/issues/events'
        }
      ]
    }
  ])

  // Computed properties
  const mobileBreakpoint = computed(() => 1350)

  const shouldAutoCollapse = computed(() => {
    return windowWidth.value < mobileBreakpoint.value
  })

  // Actions
  const toggleCollapse = () => {
    isCollapsed.value = !isCollapsed.value
    updateBodyClasses()

    // Close all submenus when collapsing
    if (isCollapsed.value) {
      activeSubmenus.value.clear()
    }
  }

  const setCollapsed = collapsed => {
    isCollapsed.value = collapsed
    updateBodyClasses()
  }

  const updateWindowWidth = width => {
    windowWidth.value = width
    isMobile.value = width < mobileBreakpoint.value

    // Auto-collapse on mobile
    if (shouldAutoCollapse.value) {
      setCollapsed(true)
    }
  }

  const toggleSubmenu = submenuId => {
    if (activeSubmenus.value.has(submenuId)) {
      activeSubmenus.value.delete(submenuId)
    } else {
      activeSubmenus.value.add(submenuId)
    }
  }

  const setActiveSubmenu = (submenuId, isActive) => {
    if (isActive) {
      activeSubmenus.value.add(submenuId)
    } else {
      activeSubmenus.value.delete(submenuId)
    }
  }

  const isSubmenuActive = submenuId => {
    return activeSubmenus.value.has(submenuId)
  }

  const updateActiveSubmenuFromRoute = currentRoute => {
    // Clear all active submenus first
    activeSubmenus.value.clear()

    // Find which submenu contains the current route
    const routeMap = {
      '/pr/': 'pull-requests',
      '/reviewers': 'pull-requests',
      '/issues/': 'issues',
      '/elements/': 'elements',
      '/components/': 'components',
      '/forms/': 'forms'
    }

    Object.entries(routeMap).forEach(([pathPrefix, submenuId]) => {
      if (currentRoute.startsWith(pathPrefix)) {
        activeSubmenus.value.add(submenuId)
      }
    })
  }

  const updateBodyClasses = () => {
    const body = document.body
    if (isCollapsed.value) {
      body.classList.add('closed-sidebar')
      if (isMobile.value) {
        body.classList.add('closed-sidebar-md')
      }
    } else {
      body.classList.remove('closed-sidebar', 'closed-sidebar-md', 'closed-sidebar-open')
    }
  }

  const initializeSidebar = currentRoute => {
    // Set initial window width
    updateWindowWidth(window.innerWidth)

    // Set active submenu based on current route
    updateActiveSubmenuFromRoute(currentRoute)

    // Apply initial body classes
    updateBodyClasses()
  }

  // Getters
  const getMenuItems = computed(() => menuItems.value)
  const getIsCollapsed = computed(() => isCollapsed.value)
  const getIsMobile = computed(() => isMobile.value)
  const getActiveSubmenus = computed(() => Array.from(activeSubmenus.value))

  return {
    // State
    isCollapsed,
    isMobile,
    activeSubmenus,
    windowWidth,
    menuItems,

    // Computed
    mobileBreakpoint,
    shouldAutoCollapse,
    getMenuItems,
    getIsCollapsed,
    getIsMobile,
    getActiveSubmenus,

    // Actions
    toggleCollapse,
    setCollapsed,
    updateWindowWidth,
    toggleSubmenu,
    setActiveSubmenu,
    isSubmenuActive,
    updateActiveSubmenuFromRoute,
    updateBodyClasses,
    initializeSidebar
  }
})
