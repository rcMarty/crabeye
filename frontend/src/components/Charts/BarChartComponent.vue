<template>
  <div class="chart-container" :style="`height: ${height}px`">
    <canvas ref="chartCanvas"></canvas>
  </div>
</template>

<script>
import { defineComponent, ref, onMounted, onBeforeUnmount, watch } from 'vue'
import { Chart, registerChartJS } from './chartSetup'

registerChartJS()

export default defineComponent({
  name: 'BarChartComponent',
  props: {
    data: {
      type: Object,
      required: true
    },
    options: {
      type: Object,
      default: () => ({})
    },
    height: {
      type: Number,
      default: 300
    },
    horizontal: {
      type: Boolean,
      default: false
    }
  },
  setup(props) {
    const chartCanvas = ref(null)
    let chartInstance = null

    const createChart = () => {
      if (!chartCanvas.value) return

      if (chartInstance) {
        chartInstance.destroy()
        chartInstance = null
      }

      const ctx = chartCanvas.value.getContext('2d')
      chartInstance = new Chart(ctx, {
        type: 'bar',
        data: props.data,
        options: {
          indexAxis: props.horizontal ? 'y' : 'x',
          ...props.options,
          responsive: true,
          maintainAspectRatio: false
        }
      })
    }

    onMounted(() => {
      createChart()
    })

    onBeforeUnmount(() => {
      if (chartInstance) {
        chartInstance.destroy()
        chartInstance = null
      }
    })

    watch(
      () => [props.data, props.horizontal],
      () => {
        createChart()
      },
      { deep: true }
    )

    return {
      chartCanvas
    }
  }
})
</script>

<style scoped>
.chart-container {
  position: relative;
  width: 100%;
}
</style>
