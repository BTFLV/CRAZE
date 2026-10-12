<script lang="ts">
  let { size = 28 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="64 64 896 896" aria-hidden="true">
  <defs>
    <linearGradient id="lg-bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#1e293b" />
      <stop offset="1" stop-color="#0b1120" />
    </linearGradient>
    <linearGradient id="lg-wire" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0" stop-color="#38bdf8" />
      <stop offset="0.5" stop-color="#a78bfa" />
      <stop offset="1" stop-color="#fb923c" />
    </linearGradient>
  </defs>
  <rect x="64" y="64" width="896" height="896" rx="200" fill="url(#lg-bg)" />
  <path
    d="M190 540 H282 L346 380 L440 680 L534 380 L628 680 L722 380 L786 540 H834"
    fill="none"
    stroke="url(#lg-wire)"
    stroke-width="64"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>
