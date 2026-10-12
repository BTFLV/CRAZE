<script lang="ts">
  import { onMount } from "svelte";
  import uPlot from "uplot";
  import { store } from "../lib/store.svelte";
  import { sig } from "../lib/format";

  let {
    data,
    series,
    xLabel = "",
    yLabel = "",
    height = 180,
    logX = false,
    integerX = false,
    points = false,
    markX = null,
    markY = null,
  }: {
    data: (number | null)[][];
    series: { label: string; color?: string; dash?: boolean }[];
    xLabel?: string;
    yLabel?: string;
    height?: number;
    logX?: boolean;
    /** Only integer ticks on the x axis (e.g. iteration numbers). */
    integerX?: boolean;
    points?: boolean;
    markX?: number | null;
    markY?: number | null;
  } = $props();

  let el: HTMLDivElement;
  let plot: uPlot | null = null;
  let width = $state(300);

  function css(n: string) {
    return getComputedStyle(document.documentElement).getPropertyValue(n).trim();
  }

  function build() {
    plot?.destroy();
    if (!el || !data.length || !data[0].length) return;
    const text = css("--muted");
    const grid = css("--border");
    const accent = css("--accent");
    const axis = (label: string): uPlot.Axis => ({
      label,
      stroke: text,
      grid: { stroke: grid, width: 1 },
      ticks: { stroke: grid, width: 1 },
      font: "11px " + css("--font"),
      labelFont: "600 11px " + css("--font"),
      labelSize: 18,
      size: 52,
      values: (_u: uPlot, splits: number[]) => splits.map((v) => (v == null ? "" : sig(v, 4))),
    });
    const opts: uPlot.Options = {
      width,
      height,
      legend: { show: series.length > 1 },
      cursor: { drag: { x: false, y: false }, points: { size: 6 } },
      scales: { x: { time: false, distr: logX ? 3 : 1 } },
      axes: [integerX ? { ...axis(xLabel), incrs: [1, 2, 5, 10, 20, 50, 100] } : axis(xLabel), axis(yLabel)],
      series: [
        {},
        ...series.map((s) => ({
          label: s.label,
          stroke: s.color ?? accent,
          width: 2,
          dash: s.dash ? [5, 4] : undefined,
          points: { show: points, size: 6, fill: s.color ?? accent },
          spanGaps: false,
        })),
      ],
      hooks: {
        draw: [
          (u) => {
            const ctx = u.ctx;
            ctx.save();
            ctx.strokeStyle = css("--warn");
            ctx.lineWidth = 1.5 * devicePixelRatio;
            ctx.setLineDash([4 * devicePixelRatio, 4 * devicePixelRatio]);
            if (markX != null) {
              const x = u.valToPos(markX, "x", true);
              ctx.beginPath();
              ctx.moveTo(x, u.bbox.top);
              ctx.lineTo(x, u.bbox.top + u.bbox.height);
              ctx.stroke();
            }
            if (markY != null) {
              const y = u.valToPos(markY, "y", true);
              ctx.beginPath();
              ctx.moveTo(u.bbox.left, y);
              ctx.lineTo(u.bbox.left + u.bbox.width, y);
              ctx.stroke();
            }
            ctx.restore();
          },
        ],
      },
    };
    plot = new uPlot(opts, data as uPlot.AlignedData, el);
  }

  onMount(() => {
    const ro = new ResizeObserver(() => {
      const w = Math.floor(el.getBoundingClientRect().width);
      if (w > 0 && w !== width) width = w;
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
      plot?.destroy();
    };
  });

  $effect(() => {
    void [data, series, width, height, markX, markY, store.theme, store.lang];
    build();
  });
</script>

<div class="chart" bind:this={el}></div>

<style>
  .chart {
    width: 100%;
    min-height: 40px;
  }
</style>
