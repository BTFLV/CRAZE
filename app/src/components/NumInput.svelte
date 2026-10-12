<script lang="ts">
  import { inputNum as num, parseNum } from "../lib/format";

  let {
    value = $bindable(),
    unit = "",
    min = -Infinity,
    max = Infinity,
    digits = 6,
    size = "",
    disabled = false,
    integer = false,
    onchange,
  }: {
    value: number;
    unit?: string;
    min?: number;
    max?: number;
    digits?: number;
    size?: "" | "sm";
    disabled?: boolean;
    integer?: boolean;
    onchange?: (v: number) => void;
  } = $props();

  let text = $state("");
  let focused = $state(false);

  $effect(() => {
    if (!focused) text = isFinite(value) ? num(value, digits) : "";
  });

  function commit() {
    let v = parseNum(text);
    if (!isFinite(v)) {
      text = num(value, digits);
      return;
    }
    if (integer) v = Math.round(v);
    v = Math.min(max, Math.max(min, v));
    value = v;
    text = num(v, digits);
    onchange?.(v);
  }
</script>

<div class="input-unit">
  <input
    class="input num {size}"
    type="text"
    inputmode="decimal"
    {disabled}
    bind:value={text}
    onfocus={() => (focused = true)}
    onblur={() => {
      focused = false;
      commit();
    }}
    onkeydown={(e) => {
      if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
    }}
  />
  {#if unit}<span class="unit">{unit}</span>{/if}
</div>
