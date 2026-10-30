<script lang="ts">
  import { commands, type Direction, type Zoom } from '#lib/bindings.js';
  import { uiState } from '#lib/ui-state.svelte.js';
  import { ask } from '@tauri-apps/plugin-dialog';
  import { on } from 'svelte/events';
  import { onDestroy } from 'svelte';

  async function confirmSetPreset(preset: number, name: string) {
    const isConfirmed = await ask(`Are you sure you want to set ${name}?`, {
      kind: 'warning',
    });

    if (isConfirmed) {
      await commands.setPreset(preset, name);
    }
  }

  const startStopCommands = {
    in: [commands.zoom.bind(null, 'in'), commands.stopZoom],
    out: [commands.zoom.bind(null, 'out'), commands.stopZoom],
    up: [commands.moveCamera.bind(null, 'up'), commands.stopMove],
    down: [commands.moveCamera.bind(null, 'down'), commands.stopMove],
    left: [commands.moveCamera.bind(null, 'left'), commands.stopMove],
    right: [commands.moveCamera.bind(null, 'right'), commands.stopMove],
  } as const;

  let finishActiveInteraction: (() => void) | null = null;

  onDestroy(() => {
    finishActiveInteraction?.();
  });

  async function onpointerdown(
    event: PointerEvent,
    direction: Zoom | Direction,
  ) {
    if (finishActiveInteraction || event.button !== 0) {
      return;
    }

    const button = event.currentTarget as HTMLButtonElement;
    const pointerId = event.pointerId;
    const [startCommand, stopCommand] = startStopCommands[direction];

    const cleanup: (() => void)[] = [];
    let isFinished = false;
    let started = Promise.resolve();

    async function stopAfterStart() {
      try {
        try {
          // Wait for the start (or resolved promise) to end before stopping
          await started;
        } catch {
          // The start error is logged below; still attempt to stop.
        }

        await stopCommand();
      } catch (error: unknown) {
        console.error('Error stopping command:', error);
      } finally {
        if (finishActiveInteraction === finish) {
          // eslint-disable-next-line unicorn/no-top-level-assignment-in-function
          finishActiveInteraction = null;
        }
      }
    }

    function finish() {
      if (isFinished) {
        return;
      }

      // Set this before releasing capture, which can trigger another event.
      isFinished = true;

      for (const off of cleanup) {
        off();
      }

      if (button.hasPointerCapture(pointerId)) {
        button.releasePointerCapture(pointerId);
      }

      void stopAfterStart();
    }

    function finishForPointer(event: PointerEvent) {
      if (event.pointerId === pointerId) {
        finish();
      }
    }

    // eslint-disable-next-line unicorn/no-top-level-assignment-in-function
    finishActiveInteraction = finish;

    cleanup.push(
      on(button, 'pointerup', finishForPointer),
      on(button, 'pointercancel', finishForPointer),
      on(button, 'lostpointercapture', finishForPointer),
      on(globalThis, 'blur', finish),
      on(globalThis, 'pagehide', finish),
      on(document, 'visibilitychange', () => {
        if (document.visibilityState === 'hidden') {
          finish();
        }
      }),
    );

    try {
      button.setPointerCapture(pointerId);
    } catch (error: unknown) {
      console.error('Error setting pointer capture:', error);
      finish();
      return;
    }

    // Retain the promise so finish() can order stop after start.
    // The callback also turns synchronous errors into rejections.

    try {
      started = startCommand();
      await started;
    } catch (error: unknown) {
      console.error('Error starting command:', error);
      finish();
    }
  }
</script>

<svelte:head>
  <title>Camera Control Settings</title>
</svelte:head>

<form class="self-center">
  <div class="flex gap-1">
    <!-- spacer to make sure the label/select is centered -->
    <div class="m-0 size-8 p-0"></div>

    <label class="inline-flex items-stretch gap-1">
      <span class="m-auto">Port:</span>
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

    <button
      type="button"
      class="m-0 size-8 p-0 text-center text-xl font-normal"
      onclick={() => void commands.refreshPorts()}>&#x21BB;</button
    >
  </div>
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
      '\u{2191}',
      'up',
      'row-start-1 col-start-5 col-span-4',
    )}
    {@render DirectionButton(
      '\u{2190}',
      'left',
      'row-start-2 col-start-1 col-span-4',
    )}
    {@render DirectionButton(
      '\u{2192}',
      'right',
      'row-start-2 col-start-9 col-span-4',
    )}
    {@render DirectionButton(
      '\u{2193}',
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
