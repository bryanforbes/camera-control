<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import '../app.css';
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

  // In order to avoid a flash of white in dark mode when opening
  // the application, the windows are created invisible and then
  // shown and focused after svelte has initialized
  onMount(() => {
    const current = WebviewWindow.getCurrent();
    setTimeout(() => {
      void current.isVisible().then((isVisible) => {
        if (!isVisible) {
          void current.show().then(() => current.setFocus());
        }
      });
    }, 50);
  });

  let { children }: { children: Snippet } = $props();
</script>

<main class="flex size-full flex-col justify-between p-2">
  {@render children()}
</main>
