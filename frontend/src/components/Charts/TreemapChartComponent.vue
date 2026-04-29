<template>
  <div class="treemap-outer">
    <!-- Click-based info panel -->
    <div v-if="selected" class="tm-info-bar">
      <span class="tm-info-name">{{ selected.path }}</span>
      <span class="tm-info-mods">{{ selected.modifications.toLocaleString() }} modifications</span>
      <button class="tm-info-close" @click="deselect">✕</button>
    </div>

    <!-- Treemap scroll container -->
    <div class="treemap-wrapper" :style="`height: ${computedHeight}px`" @click.self="deselect">
      <div ref="treemapEl" class="treemap-content"></div>
    </div>
  </div>
</template>

<script>
import { defineComponent, ref, onMounted, onBeforeUnmount, watch, nextTick } from 'vue'

export default defineComponent({
  name: 'TreemapChartComponent',
  props: {
    data: {
      type: Object,
      required: true
    },
    height: {
      type: Number,
      default: 800
    }
  },
  setup(props) {
    const treemapEl = ref(null)
    const selected = ref(null)
    const selectedEl = ref(null)
    const computedHeight = ref(props.height || 800)

    function deselect() {
      if (selectedEl.value) selectedEl.value.classList.remove('tm-selected')
      selectedEl.value = null
      selected.value = null
    }

    function countNodes(node) {
      if (!node || !node.children || node.children.length === 0) return 1
      return 1 + node.children.reduce((s, c) => s + countNodes(c), 0)
    }

    // Squarified treemap layout — produces rectangles with aspect ratios
    // as close to 1:1 as possible (same algorithm WinDirStat / KDirStat uses).
    function squarify(children, x, y, w, h) {
      if (!children || children.length === 0) return []

      const total = children.reduce((s, c) => s + c.modifications, 0)
      if (total === 0) return []

      // Sort descending by size for better layout
      const sorted = [...children].sort((a, b) => b.modifications - a.modifications)
      const rects = []

      let cx = x, cy = y, cw = w, ch = h
      let i = 0

      while (i < sorted.length) {
        // Decide layout direction: lay along the shorter side
        const vertical = cw <= ch
        const side = vertical ? cw : ch
        const remaining = sorted.slice(i).reduce((s, c) => s + c.modifications, 0)

        // Greedily add items to the current row as long as aspect ratio improves
        const row = []
        let rowArea = 0

        function worstAspect(items, itemsArea, sideLen) {
          if (sideLen === 0) return Infinity
          const s2 = sideLen * sideLen
          const a2 = itemsArea * itemsArea
          let worst = 0
          for (const item of items) {
            const r = (s2 * item.modifications) / a2
            const ar = Math.max(r, 1 / r)
            if (ar > worst) worst = ar
          }
          return worst
        }

        row.push(sorted[i])
        rowArea += sorted[i].modifications

        while (i + row.length < sorted.length) {
          const next = sorted[i + row.length]
          const newArea = rowArea + next.modifications
          const currentWorst = worstAspect(row, rowArea, side)
          const newRow = [...row, next]
          const newWorst = worstAspect(newRow, newArea, side)
          if (newWorst <= currentWorst) {
            row.push(next)
            rowArea = newArea
          } else {
            break
          }
        }

        // Layout this row
        const rowFraction = rowArea / remaining
        const rowThickness = vertical ? ch * rowFraction : cw * rowFraction

        let offset = 0
        for (const item of row) {
          const actualLen = side * (item.modifications / rowArea)

          let rx, ry, rw, rh
          if (vertical) {
            rx = cx + offset
            ry = cy
            rw = actualLen
            rh = rowThickness
          } else {
            rx = cx
            ry = cy + offset
            rw = rowThickness
            rh = actualLen
          }
          offset += actualLen

          rects.push({ node: item, x: rx, y: ry, w: rw, h: rh })
        }

        // Reduce remaining area
        if (vertical) {
          cy += rowThickness
          ch -= rowThickness
        } else {
          cx += rowThickness
          cw -= rowThickness
        }

        i += row.length
      }

      return rects
    }

    // Color palette — deeper nesting = more saturated/varied hues
    const HUES = [210, 340, 45, 160, 270, 25, 190, 310, 90, 130, 0, 60]

    function colorForLevel(level, index) {
      const hue = HUES[(level + index) % HUES.length]
      const sat = Math.max(30, 65 - level * 5)
      const light = Math.min(55, 38 + level * 4)
      return `hsl(${hue}, ${sat}%, ${light}%)`
    }

    function borderColorForLevel(level, index) {
      const hue = HUES[(level + index) % HUES.length]
      const sat = Math.max(20, 50 - level * 5)
      const light = Math.min(70, 55 + level * 4)
      return `hsl(${hue}, ${sat}%, ${light}%)`
    }

    const createTreemap = () => {
      if (!treemapEl.value) return
      treemapEl.value.innerHTML = ''
      // Reset selection — old DOM elements are gone after re-render
      selectedEl.value = null
      selected.value = null

      const data = props.data
      if (!data || !data.name) return

      // Scale height to amount of data: sqrt curve, min 600, max 2400
      const nodeCount = countNodes(data)
      const dynamicH = Math.max(600, Math.min(2400, Math.round(Math.sqrt(nodeCount) * 55)))
      computedHeight.value = dynamicH

      const container = treemapEl.value
      const containerWidth = container.parentElement?.clientWidth || container.offsetWidth || 900
      const containerHeight = dynamicH

      container.style.width = containerWidth + 'px'
      container.style.height = containerHeight + 'px'

      const HEADER_H = 22
      const MIN_RECT = 4
      const PADDING = 2

      function renderNode(node, x, y, w, h, level, siblingIdx, path) {
        if (w < MIN_RECT || h < MIN_RECT) return

        const hasChildren = node.children && node.children.length > 0

        const el = document.createElement('div')
        el.className = 'tm-node'
        el.style.position = 'absolute'
        el.style.left = x + 'px'
        el.style.top = y + 'px'
        el.style.width = w + 'px'
        el.style.height = h + 'px'
        el.style.border = `1px solid ${borderColorForLevel(level, siblingIdx)}`

        // Click handler — highlight node, show info bar with full path
        el.addEventListener('click', (e) => {
          e.stopPropagation()
          if (selectedEl.value) selectedEl.value.classList.remove('tm-selected')
          el.classList.add('tm-selected')
          selectedEl.value = el
          selected.value = { name: node.name, path, modifications: node.modifications }
        })

        if (!hasChildren) {
          // Leaf node
          el.style.backgroundColor = colorForLevel(level, siblingIdx)
          el.classList.add('tm-leaf')
          if (w > 30 && h > 16) {
            const lbl = document.createElement('span')
            lbl.className = 'tm-label'
            lbl.textContent = node.name
            lbl.style.pointerEvents = 'none'
            el.appendChild(lbl)
          }
          if (w > 50 && h > 30) {
            const cnt = document.createElement('span')
            cnt.className = 'tm-count'
            cnt.textContent = node.modifications.toLocaleString()
            cnt.style.pointerEvents = 'none'
            el.appendChild(cnt)
          }
        } else {
          // Directory node
          el.style.backgroundColor = colorForLevel(level, siblingIdx)
          el.classList.add('tm-dir')

          const showHeader = h > HEADER_H + 10 && w > 40
          let childY = y + PADDING
          let childH = h - PADDING * 2

          if (showHeader) {
            const header = document.createElement('div')
            header.className = 'tm-header'
            header.textContent = `${node.name} (${node.modifications.toLocaleString()})`
            header.style.height = HEADER_H + 'px'
            header.style.lineHeight = HEADER_H + 'px'
            header.style.pointerEvents = 'none'
            el.appendChild(header)
            childY = y + HEADER_H + 1
            childH = h - HEADER_H - 1 - PADDING
          }

          container.appendChild(el)

          const childX = x + PADDING
          const childW = w - PADDING * 2

          if (childW > MIN_RECT && childH > MIN_RECT) {
            const rects = squarify(node.children, childX, childY, childW, childH)
            rects.forEach((r, idx) => {
              renderNode(r.node, r.x, r.y, r.w, r.h, level + 1, idx, `${path}/${r.node.name}`)
            })
          }
          return // already appended
        }

        container.appendChild(el)
      }

      renderNode(data, 0, 0, containerWidth, containerHeight, 0, 0, data.name)
    }

    let resizeObserver = null

    onMounted(() => {
      nextTick(() => {
        createTreemap()
        // Re-render on container resize so widths stay correct
        const RO = /** @type {typeof ResizeObserver} */ (window.ResizeObserver)
        if (RO && treemapEl.value?.parentElement) {
          resizeObserver = new RO(() => { nextTick(createTreemap) })
          resizeObserver.observe(treemapEl.value.parentElement)
        }
      })
    })

    onBeforeUnmount(() => {
      if (resizeObserver) resizeObserver.disconnect()
      if (treemapEl.value) treemapEl.value.innerHTML = ''
    })

    watch(() => props.data, () => { nextTick(createTreemap) }, { deep: true })

    return { treemapEl, selected, selectedEl, computedHeight, deselect }
  }
})
</script>

<style scoped>
/* ── outer container ─────────────────── */
.treemap-outer {
  width: 100%;
  border-radius: 6px;
  overflow: hidden;
  background: #1a1a2e;
}

/* ── click info bar ──────────────────── */
.tm-info-bar {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 6px 12px;
  background: rgba(255,255,255,0.08);
  border-bottom: 1px solid rgba(255,255,255,0.12);
  min-height: 36px;
}

.tm-info-name {
  font-weight: 700;
  color: #fff;
  font-size: 13px;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tm-info-mods {
  font-size: 12px;
  color: #94a3b8;
  white-space: nowrap;
}

.tm-info-close {
  background: none;
  border: none;
  color: #94a3b8;
  cursor: pointer;
  font-size: 14px;
  padding: 0 4px;
  line-height: 1;
}
.tm-info-close:hover { color: #fff; }

/* ── scroll wrapper ──────────────────── */
.treemap-wrapper {
  position: relative;
  width: 100%;
  overflow: auto;
}

/* ── inner absolutely-positioned canvas ─ */
.treemap-content {
  position: relative;
}

/* ── node base ──────────────────────── */
.treemap-content :deep(.tm-node) {
  box-sizing: border-box;
  overflow: hidden;
  cursor: pointer;
}

/* Subtle highlight on hover — no z-index change */
.treemap-content :deep(.tm-node:hover) {
  outline: 2px solid rgba(255,255,255,0.5);
  outline-offset: -1px;
}

/* Selected node — thick bright outline visible even on large rects */
.treemap-content :deep(.tm-node.tm-selected) {
  outline: 4px solid #ffffff;
  outline-offset: -3px;
}

/* ── leaf ───────────────────────────── */
.treemap-content :deep(.tm-leaf) {
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  align-items: flex-start;
  padding: 4px 6px;
}

.treemap-content :deep(.tm-label) {
  font-size: 10px;
  font-weight: 600;
  color: #fff;
  text-shadow: 0 1px 3px rgba(0,0,0,.6);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 100%;
  text-align: left;
}

.treemap-content :deep(.tm-count) {
  font-size: 9px;
  color: rgba(255,255,255,.75);
  text-shadow: 0 1px 2px rgba(0,0,0,.5);
}

/* ── directory header ───────────────── */
.treemap-content :deep(.tm-header) {
  font-size: 11px;
  font-weight: 700;
  color: #fff;
  text-shadow: 0 1px 3px rgba(0,0,0,.6);
  padding: 0 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  position: relative;
  z-index: 1;
}
</style>
