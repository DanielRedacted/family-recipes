<script lang="ts">
  import { api } from '../lib/api';
  import { toast } from '../lib/state.svelte';

  let { status, lan, keepAwake, onchange }: { status: any; lan: boolean; keepAwake: boolean; onchange: (b: any) => void } = $props();
  let useIp = $state(false);

  async function trustHere() {
    try { await api('POST', '/api/setup/trust-local'); toast('This Mac now trusts Recipes — https://recipes.local works in your browsers', 'ok'); }
    catch (e: any) { toast(e.message, 'error'); }
  }
</script>

<label class="row"><input type="checkbox" checked={lan} onchange={(e) => onchange({ lan: e.currentTarget.checked })} /> Share recipes with devices on this Wi-Fi</label>

{#if lan && status}
  {#if status.other}
    <p class="small">Another Mac ({status.other}) is already sharing Recipes on this network. Phones and tablets use that one; this Mac shows its own recipes here, and takes over sharing if that Mac stops.</p>
  {/if}
  {#if status.error}<p class="err small">{status.error}</p>{/if}
  {#if status.running}
    <div class="share">
      <div class="qr" aria-label="QR code">{@html useIp ? status.qr_ip : status.qr_name}</div>
      <div class="how">
        <p><b>Open Recipes on a phone or tablet:</b> scan this code with its camera, or type the address. Nothing to set up.</p>
        <p class="mono">{useIp ? status.ip_setup_url : status.setup_url}</p>
        {#if status.ip_setup_url}
          <button class="btn ghost small" onclick={() => (useIp = !useIp)}>{useIp ? 'Use recipes.local address' : 'Device can’t find recipes.local? Use the IP address'}</button>
        {/if}
        <p class="muted small">To install it as a home-screen app that keeps working while this Mac sleeps, open <b>Settings → This device → Install as an app</b> on the phone (one-time certificate step). It then opens at <b>{status.url}</b>.</p>
      </div>
    </div>
    <label class="row"><input type="checkbox" checked={keepAwake} onchange={(e) => onchange({ keep_awake: e.currentTarget.checked })} /> Keep this Mac awake while sharing</label>
    <p class="muted small">When the Mac sleeps, other devices can still read recipes they have opened before (installed app), but can’t see changes or edit.</p>
    <details>
      <summary class="small">Details</summary>
      <p class="small">Addresses: {status.ips.join(', ') || 'none'} · Name: {status.mdns}</p>
      <p class="small mono">Certificate SHA-256: {status.ca_sha256}</p>
      <button class="btn small" onclick={trustHere}>Trust the certificate on this Mac too</button>
    </details>
  {/if}
{/if}

<style>
  .share { display: flex; gap: 16px; align-items: center; flex-wrap: wrap; }
  .qr { width: 180px; height: 180px; background: #fff; border-radius: 8px; padding: 4px; flex: none; }
  .qr :global(svg) { width: 100%; height: 100%; }
  .how { flex: 1; min-width: 220px; display: grid; gap: 8px; }
  .how p { margin: 0; }
  .mono { font-family: var(--mono); font-size: 13px; overflow-wrap: anywhere; }
  .err { color: var(--danger); }
  details p { margin: 8px 0; }
</style>
