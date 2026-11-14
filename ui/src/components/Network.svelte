<script lang="ts">
  let { status, lan, keepAwake, onchange }: { status: any; lan: boolean; keepAwake: boolean; onchange: (b: any) => void } = $props();
</script>

<label class="row"><input type="checkbox" checked={lan} onchange={(e) => onchange({ lan: e.currentTarget.checked })} /> Share recipes on this network</label>
{#if !status}<p class="muted small">Network sharing is not running.</p>{/if}
