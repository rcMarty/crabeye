<template>
  <div class="treemap-container" :style="`height: ${height}px`">
    <div ref="treemapEl" class="treemap-content"></div>
  </div>
</template>

<script>
import { defineComponent, ref, onMounted, onBeforeUnmount, watch } from 'vue'

export default defineComponent({
  name: 'TreemapChartComponent',
  props: {
    data: {
      type: Object,
      required: true
    },
    height: {
      type: Number,
      default: 500
    }
  },
  setup(props) {
    const treemapEl = ref(null)

    const createTreemap = () => {
      if (!treemapEl.value) return

      // Clear previous content
      treemapEl.value.innerHTML = ''

      const data = props.data

      // Create treemap visualization
      const container = treemapEl.value
      const containerWidth = container.offsetWidth
      const containerHeight = props.height

      // Simple treemap layout function
      const drawTreemap = (node, x, y, width, height, level = 0) => {
        if (!node || width < 10 || height < 10) return

        const hasChildren = node.children && node.children.length > 0

        const div = document.createElement('div')
        div.className = 'treemap-item'
        div.title = `${node.name} (${node.modifications} modifications)` // Tooltip on hover
        div.style.position = 'absolute'
        div.style.left = `${x}px`
        div.style.top = `${y}px`
        div.style.width = `${width}px`
        div.style.height = `${height}px`
        div.style.border = '1px solid #fff'
        div.style.boxSizing = 'border-box'
        div.style.overflow = 'hidden'
        div.style.display = 'flex'
        div.style.flexDirection = 'column'
        div.style.padding = '4px'
        div.style.zIndex = hasChildren ? level : level + 1000 // Leaf nodes on top

        // Color based on level
        const colors = [
          'rgba(54, 162, 235, 0.7)',
          'rgba(255, 99, 132, 0.7)',
          'rgba(255, 206, 86, 0.7)',
          'rgba(75, 192, 192, 0.7)',
          'rgba(153, 102, 255, 0.7)',
          'rgba(255, 159, 64, 0.7)'
        ]
        div.style.backgroundColor = colors[level % colors.length]

        // Only show text for leaf nodes (nodes without children) or if height is large enough
        if (!hasChildren || height > 50) {
          const label = document.createElement('div')
          label.style.fontWeight = 'bold'
          label.style.fontSize = '12px'
          label.style.marginBottom = '2px'
          label.style.overflow = 'hidden'
          label.style.textOverflow = 'ellipsis'
          label.style.whiteSpace = 'nowrap'
          label.style.color = '#fff'
          label.style.textShadow = '1px 1px 2px rgba(0,0,0,0.8)'
          label.style.zIndex = '10'
          label.style.position = 'relative'
          label.textContent = node.name

          const count = document.createElement('div')
          count.style.fontSize = '10px'
          count.style.opacity = '0.9'
          count.style.color = '#fff'
          count.style.textShadow = '1px 1px 2px rgba(0,0,0,0.8)'
          count.style.zIndex = '10'
          count.style.position = 'relative'
          count.textContent = `${node.modifications} modifications`

          div.appendChild(label)
          div.appendChild(count)
        }

        container.appendChild(div)

        // If has children, recursively draw them
        if (hasChildren) {
          const totalMods = node.children.reduce((sum, child) => sum + child.modifications, 0)
          const padding = 2
          const headerOffset = 45 // Reserve space at top for parent container visual separation
          let currentY = y + headerOffset
          const availableHeight = height - headerOffset

          node.children.forEach(child => {
            const childHeight = (child.modifications / totalMods) * availableHeight
            if (childHeight > 10) {
              drawTreemap(child, x + padding, currentY + padding, width - padding * 2, childHeight - padding * 2, level + 1)
              currentY += childHeight
            }
          })
        }
      }

      // Start drawing
      if (data.name) {
        // It's a tree structure
        drawTreemap(data, 0, 0, containerWidth, containerHeight, 0)
      }
    }

    onMounted(() => {
      createTreemap()
    })

    onBeforeUnmount(() => {
      if (treemapEl.value) {
        treemapEl.value.innerHTML = ''
      }
    })

    watch(
      () => props.data,
      () => {
        createTreemap()
      },
      { deep: true }
    )

    return {
      treemapEl
    }
  }
})
</script>

<style scoped>
.treemap-container {
  position: relative;
  width: 100%;
  background: #f5f5f5;
  border-radius: 4px;
  overflow: hidden;
}

.treemap-content {
  position: relative;
  width: 100%;
  height: 100%;
}

.treemap-item {
  cursor: pointer;
  transition: opacity 0.2s, transform 0.1s;
}

.treemap-item:hover {
  opacity: 0.9;
  transform: scale(1.02);
  z-index: 9999 !important;
}
</style>
