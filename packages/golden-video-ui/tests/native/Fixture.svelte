<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { emit } from '@tauri-apps/api/event';
	import { NativePreview, createDesktopPreviewTransport, createPreviewSession } from '../../index';
	import type { PreviewRequest } from '../../generated/PreviewRequest';

	const desktop = createDesktopPreviewTransport();
	let latest: PreviewRequest | null = null;
	let lastLayout: Extract<PreviewRequest, { kind: 'Layout' }> | null = null;
	const session = createPreviewSession({
		available: desktop.available,
		async request(request) {
			if (request.kind === 'Layout') lastLayout = request;
			latest = request;
			return desktop.request(request);
		}
	});
	let greenVisible = $state(true);
	let redMounted = $state(true);
	let greenMounted = $state(true);
	let overlayClicks = $state(0);
	let scroller: HTMLDivElement;
	const pause = (delay: number) => new Promise((resolve) => setTimeout(resolve, delay));
	const report = {
		phase: 'starting',
		trace: [] as unknown[],
		error: null as string | null,
		maxRetirementMasks: 0
	};
	const global = window as typeof window & { __previewSmoke: typeof report };
	global.__previewSmoke = report;

	async function record(phase: string) {
		await tick();
		session.invalidate();
		await session.whenSettled();
		if (!session.state.installed)
			throw new Error(`Native installation was not confirmed during ${phase}`);
		const retirementMasks = document.querySelectorAll('[data-native-preview-retirement]').length;
		if (retirementMasks !== 0)
			throw new Error(`Retirement masks remained after confirmed ${phase} layout`);
		report.phase = phase;
		report.trace.push({
			phase,
			state: { ...session.state },
			views: lastLayout?.views ?? [],
			retirementMasks,
			request: latest,
			overlayClicks
		});
		if (desktop.available()) await emit('golden-preview-smoke', report);
		await pause(250);
	}

	onMount(() => {
		let cancelled = false;
		const masks = new MutationObserver(() => {
			report.maxRetirementMasks = Math.max(
				report.maxRetirementMasks,
				document.querySelectorAll('[data-native-preview-retirement]').length
			);
			if (report.maxRetirementMasks > 64) {
				report.phase = 'failed';
				report.error = 'Retirement mask capacity exceeded';
			}
		});
		masks.observe(document.body, { subtree: true, childList: true });
		void (async () => {
			try {
				if (!(await session.connect())) throw new Error('Native preview capability unavailable');
				await record('initial');
				document.querySelector<HTMLButtonElement>('[data-overlay-control]')!.click();
				if (overlayClicks !== 1) throw new Error('DOM overlay input did not run');
				scroller.scrollLeft = scroller.clientWidth / 6;
				scroller.dispatchEvent(new Event('scroll'));
				await record('clipped');
				greenVisible = false;
				await record('hidden');
				redMounted = false;
				await record('unregistered');
				greenMounted = false;
				await record('empty');
				await session.dispose();
				if (!session.state.installed || session.state.error)
					throw new Error('Native preview disposal did not confirm a cleared frame');
				if (!cancelled) {
					report.phase = 'done';
					await emit('golden-preview-smoke', report);
				}
			} catch (error) {
				report.error = String(error);
				report.phase = 'failed';
				if (desktop.available()) await emit('golden-preview-smoke', report);
			}
		})();
		return () => {
			cancelled = true;
			masks.disconnect();
			void session.dispose();
		};
	});
</script>

<section class="chrome">
	<h1>Golden native preview fixture</h1>
	<p>Native image regions with DOM and SVG controls.</p>
</section>
<div class="scroller" bind:this={scroller}>
	<div class="views">
		{#if redMounted}
			<div class="preview red">
				<NativePreview {session} source="red" label="Red native source">
					<svg viewBox="0 0 100 80" aria-hidden="true"><path d="M5 5H95V75H5Z" /></svg>
				</NativePreview>
				<button
					type="button"
					data-overlay-control
					onclick={() => overlayClicks++}
					aria-label="DOM overlay control">+</button
				>
			</div>
		{/if}
		{#if greenMounted}
			<div class="preview green">
				<NativePreview
					{session}
					source="green"
					visible={greenVisible}
					label="Green native source"
				/>
			</div>
		{/if}
	</div>
</div>
<p class="chrome">Overlay clicks: {overlayClicks}</p>

<style>
	:global(html),
	:global(body),
	:global(#app) {
		margin: 0;
		min-height: 100%;
		background: transparent;
		color: #f2f4f7;
		font-family: sans-serif;
	}
	.chrome {
		position: relative;
		margin: 1em;
		padding: 0.5em;
		background: #242934;
	}
	h1 {
		margin: 0;
		font-size: 1.1em;
	}
	p {
		margin-block: 0.5em 0;
	}
	.scroller {
		margin: 1.5em;
		width: 25em;
		height: 10em;
		max-width: 85vw;
		overflow: hidden;
	}
	.views {
		display: flex;
		align-items: start;
		gap: 1em;
		width: 30em;
	}
	.preview {
		position: relative;
		flex: none;
		height: 8em;
	}
	.red {
		width: 12em;
	}
	.green {
		width: 14em;
	}
	svg {
		width: 100%;
		height: 100%;
		fill: none;
		stroke: white;
		stroke-width: 1;
	}
	button {
		position: absolute;
		inset: 1em auto auto 1em;
		width: 2em;
		height: 2em;
		border: 0.1em solid white;
		border-radius: 0.2em;
		background: #263146;
		color: white;
		font-size: 1em;
	}
</style>
