<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { LogicalSize } from '@tauri-apps/api/dpi';

  import '../app.css';

  let content: HTMLElement;

  async function show(current: WebviewWindow) {
    // Resize the window to fit the content
    const innerSize = await current.innerSize();
    const size = innerSize.toLogical(await current.scaleFactor());
    const heightOffset = size.height - window.innerHeight;
    const contentHeight = Math.ceil(content.getBoundingClientRect().height);

    await current.setSize(
      new LogicalSize(size.width, contentHeight + heightOffset),
    );

    const isVisible = await current.isVisible();

    if (isVisible) {
      return;
    }

    await current.show();
    await current.setFocus();
  }

  // In order to avoid a flash of white in dark mode when opening
  // the application, the windows are created invisible and then
  // shown and focused after svelte has initialized
  onMount(() => {
    const current = WebviewWindow.getCurrent();
    setTimeout(() => {
      void show(current);
    }, 50);
  });

  let { children }: { children: Snippet } = $props();
</script>

<main bind:this={content} class="flex h-auto w-full flex-col gap-6 p-2">
  {@render children()}
</main>
