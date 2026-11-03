<script lang="ts">
  import { commands } from '#lib/bindings.js';
  import { uiState } from '#lib/ui-state.svelte.js';
  import CircleX from '@lucide/svelte/icons/circle-x';

  async function goToPreset(preset: number, name: string) {
    await commands.goToPreset(preset, name);
  }

  async function openSettings() {
    await commands.openSettings();
  }

  function tooltipUpdater(node: HTMLElement) {
    function update() {
      node.title =
        node.scrollWidth > node.clientWidth ? uiState.status.text : '';
    }

    $effect(() => {
      // eslint-disable-next-line @typescript-eslint/no-unused-expressions
      uiState.status.text;
      // eslint-disable-next-line @typescript-eslint/no-unused-expressions
      uiState.status.isError;

      update();
    });

    const observer = new ResizeObserver(update);
    observer.observe(node);

    return () => observer.disconnect();
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
  inert={!uiState.port}
>
  <button type="button" class="col-start-1" inert>Power on</button>
  <button type="button" class="col-start-1 row-start-2" inert>Power off</button>

  <button type="button" class="col-start-3" inert>Autofocus on</button>
  <button type="button" class="col-start-3 row-start-2" inert>
    Autofocus off
  </button>
</section>

<section class="flex flex-col gap-1" inert={!uiState.port}>
  {@render PresetButton(1, 'Sanctuary')}
  {@render PresetButton(2, 'Stage')}
  {@render PresetButton(3, 'Speaker')}
  {@render PresetButton(4, 'Baptistry')}
</section>

<footer class="flex flex-row items-end justify-between gap-1">
  <p class="truncate" {@attach tooltipUpdater}>
    {#if uiState.status.isError}<CircleX
        class="mt-1 mb-1 inline-block size-4 align-top text-red-600"
      />{/if}
    {uiState.status.text}
  </p>
  <button type="button" onclick={() => openSettings()}>Settings</button>
</footer>
