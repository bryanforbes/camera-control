<script lang="ts">
  import { commands, type Direction, type Zoom } from '#lib/bindings.js';
  import { uiState } from '#lib/ui-state.svelte.js';
  import type { LucideProps } from '@lucide/svelte';
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import ArrowRight from '@lucide/svelte/icons/arrow-right';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import Minus from '@lucide/svelte/icons/minus';
  import Plus from '@lucide/svelte/icons/plus';
  import Refresh from '@lucide/svelte/icons/refresh-cw';
  import { ask } from '@tauri-apps/plugin-dialog';
  import { onDestroy, type Component } from 'svelte';
  import { on } from 'svelte/events';

  async function selectPort(select: HTMLSelectElement) {
    try {
      await commands.setPort(select.value === '' ? null : select.value);
    } catch (error: unknown) {
      console.error('Error switching camera port:', error);
    } finally {
      select.value = uiState.port ?? '';
    }
  }

  async function confirmSetPreset(preset: number, name: string) {
    const isConfirmed = await ask(`Are you sure you want to set ${name}?`, {
      kind: 'warning',
    });

    if (isConfirmed) {
      await commands.setPreset(preset, name);
    }
  }

  const startCommands = {
    in: commands.zoom.bind(null, 'in'),
    out: commands.zoom.bind(null, 'out'),
    up: commands.moveCamera.bind(null, 'up'),
    down: commands.moveCamera.bind(null, 'down'),
    left: commands.moveCamera.bind(null, 'left'),
    right: commands.moveCamera.bind(null, 'right'),
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
    const startCommand = startCommands[direction];

    const cleanup: (() => void)[] = [];
    let isFinished = false;
    let registration: Promise<string> | undefined;

    async function stopAfterRegistration() {
      try {
        if (!registration) {
          return;
        }

        let token: string;
        try {
          token = await registration;
        } catch {
          // The registration error is reported by the start handler.
          return;
        }

        await commands.stopMotion(token);
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

      void stopAfterRegistration();
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

    try {
      registration = startCommand();
      await registration;
    } catch (error: unknown) {
      console.error('Error registering motion:', error);
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

    <label class="inline-flex items-stretch gap-1">
      <span class="m-auto">Port:</span>
      <select
        id="ports"
        value={uiState.port ?? ''}
        onchange={(event) => void selectPort(event.currentTarget)}
      >
        <option value=""></option>
        {#if uiState.ports}
          {#each uiState.ports as port (port)}
            <option value={port}>{port}</option>
          {/each}
        {/if}
      </select>
    </label>

    <button type="button" onclick={() => void commands.refreshPorts()}
      ><Refresh class="mt-1 mb-1 size-3" /></button
    >
  </div>
</form>

{#snippet PresetButton(preset: number, name: string)}
  <button type="button" onclick={() => void confirmSetPreset(preset, name)}
    >Set {name}</button
  >
{/snippet}

{#snippet DirectionButton(
  Icon: Component<LucideProps>,
  direction: Direction | Zoom,
  classes: string,
)}
  <button
    type="button"
    class={['flex', 'items-center', 'justify-center', classes]}
    onpointerdown={(event) => void onpointerdown(event, direction)}
  >
    <Icon />
  </button>
{/snippet}

<div class="flex flex-row justify-between gap-1 p-4" inert={!uiState.port}>
  <section
    class="grid grid-cols-[repeat(12,25px)] grid-rows-[100px_100px_100px_50px] gap-1"
  >
    {@render DirectionButton(
      ArrowUp,
      'up',
      'row-start-1 col-start-5 col-span-4',
    )}
    {@render DirectionButton(
      ArrowLeft,
      'left',
      'row-start-2 col-start-1 col-span-4',
    )}
    {@render DirectionButton(
      ArrowRight,
      'right',
      'row-start-2 col-start-9 col-span-4',
    )}
    {@render DirectionButton(
      ArrowDown,
      'down',
      'row-start-3 col-start-5 col-span-4',
    )}

    {@render DirectionButton(
      Minus,
      'out',
      'row-start-4 col-start-2 col-span-2',
    )}
    {@render DirectionButton(Plus, 'in', 'row-start-4 col-start-10 col-span-2')}
  </section>

  <section class="flex flex-col justify-between gap-1">
    {@render PresetButton(1, 'Sanctuary')}
    {@render PresetButton(2, 'Stage')}
    {@render PresetButton(3, 'Speaker')}
    {@render PresetButton(4, 'Baptistry')}
  </section>
</div>
