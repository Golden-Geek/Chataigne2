<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "./native";
  import { observeEditorIsolation } from "./editor-isolation";
  import type { CssRect, ProbeLayout, ProbeStatus } from "./generated";

  let stage: HTMLDivElement;
  let cards: HTMLDivElement[] = [];
  let dialog: HTMLDialogElement;
  let visible = $state(true);
  let wide = $state(false);
  let menu = $state(false);
  let zoom = $state(1);
  let status = $state<ProbeStatus>();
  let layoutSession = $state<number>();
  let physicalPerRem = $state(16);
  let error = $state("");
  let offsets = $state([
    { x: 5, y: 6 },
    { x: 30, y: 34 },
  ]);
  let drag:
    | {
        index: number;
        x: number;
        y: number;
        initialX: number;
        initialY: number;
      }
    | undefined;
  let revision = 0;
  let pendingFrame = 0;

  function bounds(rect: DOMRect): CssRect {
    return {
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
    };
  }

  function publish() {
    pendingFrame = 0;
    if (!stage || layoutSession === undefined) return;
    physicalPerRem =
      window.devicePixelRatio *
      parseFloat(getComputedStyle(document.documentElement).fontSize);
    const viewport = stage.getBoundingClientRect();
    const viewportLeft = viewport.left + stage.clientLeft;
    const viewportTop = viewport.top + stage.clientTop;
    const layout: ProbeLayout = {
      session: layoutSession,
      revision: ++revision,
      device_pixel_ratio: window.devicePixelRatio,
      rects: visible
        ? cards.map((card) => {
            const rect = card.getBoundingClientRect();
            const left = Math.max(0, rect.left, viewportLeft);
            const top = Math.max(0, rect.top, viewportTop);
            const right = Math.min(
              window.innerWidth,
              rect.right,
              viewportLeft + stage.clientWidth,
            );
            const bottom = Math.min(
              window.innerHeight,
              rect.bottom,
              viewportTop + stage.clientHeight,
            );
            return {
              bounds: bounds(rect),
              clip: {
                left,
                top,
                width: Math.max(0, right - left),
                height: Math.max(0, bottom - top),
              },
            };
          })
        : [],
    };
    void invoke("update_layout", { layout }).catch(
      (cause) => (error = String(cause)),
    );
  }

  function schedule() {
    if (!pendingFrame) pendingFrame = requestAnimationFrame(publish);
  }

  function begin(index: number, event: PointerEvent) {
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag = {
      index,
      x: event.clientX,
      y: event.clientY,
      initialX: offsets[index].x,
      initialY: offsets[index].y,
    };
  }

  function move(event: PointerEvent) {
    if (!drag) return;
    const parent = cards[drag.index].parentElement!;
    offsets[drag.index] = {
      x: drag.initialX + ((event.clientX - drag.x) / parent.clientWidth) * 100,
      y: drag.initialY + ((event.clientY - drag.y) / parent.clientHeight) * 100,
    };
    schedule();
  }

  async function changeZoom() {
    try {
      await invoke("set_zoom", { factor: zoom });
      schedule();
    } catch (cause) {
      error = String(cause);
    }
  }

  $effect(() => {
    visible;
    wide;
    menu;
    layoutSession;
    schedule();
  });

  onMount(() => {
    const stopIsolation = observeEditorIsolation(() => layoutSession, schedule);
    const observer = new ResizeObserver(schedule);
    observer.observe(stage);
    cards.forEach((card) => observer.observe(card));
    stage.addEventListener("scroll", schedule);
    window.addEventListener("resize", schedule);
    let alive = true;
    const pollStatus = () => {
      void invoke<ProbeStatus>("probe_status")
        .then((value) => {
          if (alive) {
            // Bind this document once; a late old page must not adopt a replacement's token.
            if (layoutSession === undefined && value.layout_session > 0) layoutSession = value.layout_session;
            status = value;
          }
        })
        .catch((cause) => {
          if (alive) error = String(cause);
        });
    };
    pollStatus();
    const poll = setInterval(pollStatus, 500);
    schedule();
    return () => {
      alive = false;
      stopIsolation();
      observer.disconnect();
      stage.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
      clearInterval(poll);
      cancelAnimationFrame(pendingFrame);
    };
  });
</script>

<svelte:head><title>MapGyver native feasibility</title></svelte:head>

<header>
  <strong>MapGyver · M01 native underlay</strong>
  <span>{#if status}Generation {status.generation} · {status.settling ? "settling" : "rendering"} · QA open{:else}Native QA open{/if}</span>
</header>
<nav aria-label="Probe controls">
  <button onclick={() => (visible = !visible)}
    >{visible ? "Hide preview tab" : "Show preview tab"}</button
  >
  <button onclick={() => (wide = !wide)}>Change dock width</button>
  <button onclick={() => dialog.showModal()}>Dialog overlay</button>
  <button onclick={() => (menu = !menu)}>Menu overlay</button>
  <label
    >Zoom <select bind:value={zoom} onchange={changeZoom}
      ><option value={0.75}>75%</option><option value={1}>100%</option><option
        value={1.25}>125%</option
      ><option value={1.5}>150%</option><option value={2}>200%</option></select
    ></label
  >
</nav>
<main class:wide>
  <aside>
    <h1>Two native sources</h1>
    <p>
      Drag either DOM handle beyond its preview and release. Scroll, resize,
      switch the preview tab, and move this window between monitors.
    </p>
    <p>
      Source A changes from red to green, disappears, then returns blue. Source
      B stays yellow. The separate native window and Spout sender use their own
      output path.
    </p>
    <p>
      Check opaque chrome, menus and dialogs cover native pixels. Hidden
      previews must leave no stale pixels.
    </p>
    {#if status}
      <dl>
        <dt>Layout / rendered</dt>
        <dd>{status.layout_revision} / {status.rendered_revision}</dd>
        <dt>Host / browser scale</dt>
        <dd>
          {status.webview_scale.toFixed(3)} / {status.browser_scale.toFixed(3)}
        </dd>
        <dt>Preview presents</dt>
        <dd>{status.preview_presents}</dd>
        <dt>Native output presents</dt>
        <dd>{status.output_presents}</dd>
        <dt>Spout sends</dt>
        <dd>{status.spout_sends}</dd>
      </dl>
    {/if}
    {#if error || status?.error}<p role="alert">
        {error || status?.error}
      </p>{/if}
  </aside>
  <div class="stage" bind:this={stage}>
    <div class="canvas">
      {#each [0, 1] as index}
        <div
          class="preview"
          class:second={index === 1}
          class:hidden={!visible}
          style:left={`${offsets[index].x}%`}
          style:top={`${offsets[index].y}%`}
          style:width={status?.preview_extents
            ? `${status.preview_extents[index][0] / physicalPerRem}rem`
            : undefined}
          style:height={status?.preview_extents
            ? `${status.preview_extents[index][1] / physicalPerRem}rem`
            : undefined}
          bind:this={cards[index]}
        >
          <button
            class="handle"
            onpointerdown={(event) => begin(index, event)}
            onpointermove={move}
            onpointerup={() => (drag = undefined)}
            onpointercancel={() => (drag = undefined)}
            onlostpointercapture={() => (drag = undefined)}
          >
            Drag source {index === 0 ? "A" : "B"}
          </button>
          <span class="corner tl"></span><span class="corner tr"></span><span
            class="corner bl"
          ></span><span class="corner br"></span>
        </div>
      {/each}
      {#if menu}<div class="menu">
          <strong>Opaque DOM menu</strong>
          <p>Native pixels must stay underneath.</p>
          <button onclick={() => (menu = false)}>Close menu</button>
        </div>{/if}
    </div>
  </div>
</main>
<footer>
  Closing either probe window requests worker cleanup. The probe also stops at
  its configured duration.
</footer>
<dialog bind:this={dialog}>
  <h2>Opaque dialog above native video</h2>
  <p>Check alpha edges and occlusion manually.</p>
  <button onclick={() => dialog.close()}>Close</button>
</dialog>

<style>
  :global(html),
  :global(body),
  :global(#app) {
    margin: 0;
    width: 100%;
    height: 100%;
    background: transparent;
    color: #edf1f5;
    font:
      1rem system-ui,
      sans-serif;
  }
  :global(*) {
    box-sizing: border-box;
  }
  header,
  nav,
  aside,
  footer {
    background: #20262e;
  }
  header {
    padding: 1rem;
    display: flex;
    gap: 2rem;
    justify-content: space-between;
  }
  header span,
  footer {
    font-size: 0.8rem;
    color: #b8c1cc;
  }
  nav {
    display: flex;
    flex-wrap: wrap;
    gap: 0.6rem;
    padding: 0.7rem 1rem;
  }
  button,
  select {
    font: inherit;
    color: inherit;
    background: #384657;
    border: 0.07rem solid #60738b;
    border-radius: 0.3rem;
    padding: 0.45rem 0.7rem;
  }
  main {
    display: grid;
    grid-template-columns: 19rem minmax(0, 1fr);
    height: calc(100vh - 11rem);
  }
  main.wide {
    grid-template-columns: 26rem minmax(0, 1fr);
  }
  aside {
    padding: 1rem;
    overflow: auto;
  }
  h1 {
    font-size: 1.2rem;
  }
  p {
    line-height: 1.45;
  }
  dl {
    font-size: 0.85rem;
  }
  dd {
    margin: 0 0 0.65rem;
    color: #8ddde3;
  }
  .stage {
    overflow: auto;
    position: relative;
    background: transparent;
  }
  .canvas {
    position: relative;
    min-height: 65rem;
    width: 100%;
    background: transparent;
  }
  .preview {
    position: absolute;
    width: 70%;
    height: 15rem;
    background: transparent;
    outline: 0.12rem solid #f6fbff;
  }
  .preview.second {
    width: 55%;
    height: 12rem;
  }
  .preview.hidden {
    visibility: hidden;
  }
  .handle {
    position: absolute;
    top: 0.8rem;
    left: 0.8rem;
    touch-action: none;
    cursor: grab;
  }
  .corner {
    position: absolute;
    width: 0.8rem;
    height: 0.8rem;
    background: white;
    border: 0.1rem solid #252525;
  }
  .tl {
    top: 0;
    left: 0;
  }
  .tr {
    top: 0;
    right: 0;
  }
  .bl {
    bottom: 0;
    left: 0;
  }
  .br {
    bottom: 0;
    right: 0;
  }
  .menu {
    position: absolute;
    top: 4rem;
    left: 4rem;
    right: 4rem;
    padding: 1.5rem;
    z-index: 10;
    background: #394654;
  }
  dialog {
    background: #283441;
    color: #fff;
    border: 0.1rem solid #abc;
    border-radius: 0.6rem;
    padding: 2rem;
  }
  dialog::backdrop {
    background: #0008;
  }
  footer {
    padding: 0.7rem 1rem;
  }
</style>
