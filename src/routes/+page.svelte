<script lang="ts">
  import { invoke } from '$lib/common';
  import { ui_state } from '$lib/ui_state.svelte';

  async function goToPreset(preset: number, name: string) {
    await invoke('go_to_preset', { preset, name });
  }

  async function openSettings() {
    await invoke('open_settings');
  }
</script>

<svelte:head>
  <title>Camera Control</title>
</svelte:head>

{#snippet PresetButton(preset: number, name: string)}
  <button type="button" onclick={() => goToPreset(preset, name)}>{name}</button>
{/snippet}

<section
  class="grid grid-cols-[auto_1fr_auto] grid-rows-[auto] gap-1"
  inert={!ui_state.port}
>
  <button type="button" class="col-start-1">Power on</button>
  <button type="button" class="col-start-1 row-start-2">Power off</button>

  <button type="button" class="col-start-3">Autofocus on</button>
  <button type="button" class="col-start-3 row-start-2"> Autofocus off </button>
</section>

<section class="flex flex-col gap-1" inert={!ui_state.port}>
  {@render PresetButton(1, 'Sanctuary')}
  {@render PresetButton(2, 'Stage')}
  {@render PresetButton(3, 'Speaker')}
  {@render PresetButton(4, 'Baptistry')}
</section>

<footer class="flex flex-row items-end justify-between">
  <p>{ui_state.status}</p>
  <button type="button" onclick={() => openSettings()}>Settings</button>
</footer>
