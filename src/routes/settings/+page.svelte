<script lang="ts">
  import { commands } from '$lib/bindings';
  import { uiState } from '$lib/ui-state.svelte';
  import { ask } from '@tauri-apps/plugin-dialog';
  import { on } from 'svelte/events';

  async function confirmSetPreset(preset: number, name: string) {
    const confirmed = await ask(`Are you sure you want to set ${name}?`, {
      kind: 'warning',
    });

    if (confirmed) {
      await commands.setPreset(preset, name);
    }
  }

  async function onpointerdown(
    event: PointerEvent,
    direction: 'in' | 'out' | 'up' | 'down' | 'left' | 'right',
  ) {
    const button = event.target as HTMLButtonElement;

    const isZoom = direction === 'in' || direction === 'out';
    const startCommand = isZoom ? 'zoom' : 'moveCamera';
    const stopCommand = isZoom ? 'stopZoom' : 'stopMove';

    await commands[startCommand](direction);

    const onpointerup = async (event: PointerEvent) => {
      try {
        await commands[stopCommand]();
      } finally {
        off();
        button.releasePointerCapture(event.pointerId);
      }
    };

    const off = on(button, 'pointerup', (event) => void onpointerup(event));

    button.setPointerCapture(event.pointerId);
  }
</script>

<svelte:head>
  <title>Camera Control Settings</title>
</svelte:head>

<form class="self-center">
  <label>
    Port:
    <select
      id="ports"
      bind:value={
        () => uiState.port ?? '',
        (value: string) => void commands.setPort(value === '' ? null : value)
      }
    >
      <option value=""></option>
      {#if uiState.ports}
        {#each uiState.ports as port (port)}
          <option value={port}>{port}</option>
        {/each}
      {/if}
    </select>
  </label>
</form>

{#snippet PresetButton(preset: number, name: string)}
  <button type="button" onclick={() => void confirmSetPreset(preset, name)}
    >Set {name}</button
  >
{/snippet}

{#snippet DirectionButton(
  label: string,
  direction: 'in' | 'out' | 'up' | 'down' | 'left' | 'right',
  classes: string,
)}
  <button
    type="button"
    class={classes}
    onpointerdown={(event) => void onpointerdown(event, direction)}
    >{label}</button
  >
{/snippet}

<div class="flex flex-row justify-between gap-1 p-4" inert={!uiState.port}>
  <section
    class="grid grid-cols-[repeat(12,25px)] grid-rows-[100px_100px_100px_50px] gap-1"
  >
    {@render DirectionButton(
      '\u2191',
      'up',
      'row-start-1 col-start-5 col-span-4',
    )}
    {@render DirectionButton(
      '\u2190',
      'left',
      'row-start-2 col-start-1 col-span-4',
    )}
    {@render DirectionButton(
      '\u2192',
      'right',
      'row-start-2 col-start-9 col-span-4',
    )}
    {@render DirectionButton(
      '\u2193',
      'down',
      'row-start-3 col-start-5 col-span-4',
    )}

    {@render DirectionButton('-', 'out', 'row-start-4 col-start-2 col-span-2')}
    {@render DirectionButton('+', 'in', 'row-start-4 col-start-10 col-span-2')}
  </section>

  <section class="flex flex-col justify-between gap-1">
    {@render PresetButton(1, 'Sanctuary')}
    {@render PresetButton(2, 'Stage')}
    {@render PresetButton(3, 'Speaker')}
    {@render PresetButton(4, 'Baptistry')}
  </section>
</div>
