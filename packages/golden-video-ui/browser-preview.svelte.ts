import { getContext, setContext } from 'svelte';
import type { UiBrowserPreview, UiBrowserPreviewExtent, UiBrowserPreviewQuality } from 'golden_ui';

export type BrowserPreviewMode = 'disabled' | 'auto' | UiBrowserPreviewQuality;

const CONTEXT = Symbol('browser-preview');
const FRAME_INTERVAL = { low: 125, high: 67 } satisfies Record<UiBrowserPreviewQuality, number>;
const REQUEST_INTERVAL = { low: 100, high: 50 } satisfies Record<UiBrowserPreviewQuality, number>;
const DOWNGRADE_MS = 150;
const UPGRADE_MS = 95;
const UPGRADE_COOLDOWN_MS = 2000;
const UPGRADE_SAMPLES = 8;
const REQUEST_TIMEOUT_MS = 1200;

type RenderFrame = (image: ImageBitmap, extent: UiBrowserPreviewExtent) => void;

type Demand = {
	source: string;
	request: UiBrowserPreview;
	listeners: Set<RenderFrame>;
	after: string | null;
	due: number;
};

function browserPreviewSession() {
	let mode = $state<BrowserPreviewMode>('auto');
	let automaticQuality = $state<UiBrowserPreviewQuality>('high');
	let quality = $derived(mode === 'disabled' ? null : mode === 'auto' ? automaticQuality : mode);
	let highBytes = 0;
	let fastSamples = 0;
	let evaluateUpgradeAfter = 0;
	let nextLinkProbe = 0;
	let nextRequestAt = 0;
	const demands = new Map<string, Demand>();
	let active: { demand: Demand; abort: AbortController } | null = null;
	let timer: ReturnType<typeof setTimeout> | undefined;

	function observe(requested: UiBrowserPreviewQuality, elapsed: number, bytes: number) {
		if (mode !== 'auto') return;
		if (requested === 'high') {
			if (bytes) highBytes = bytes;
			if (elapsed >= DOWNGRADE_MS) {
				automaticQuality = 'low';
				evaluateUpgradeAfter = performance.now() + UPGRADE_COOLDOWN_MS;
				fastSamples = 0;
			}
		} else {
			// Reserve headroom for edits even when small low-quality frames transfer quickly.
			const sizeRatio = highBytes && bytes ? Math.max(2, highBytes / bytes) : 4;
			fastSamples = bytes && elapsed * sizeRatio <= UPGRADE_MS ? fastSamples + 1 : 0;
			if (performance.now() >= evaluateUpgradeAfter && fastSamples >= UPGRADE_SAMPLES) {
				automaticQuality = 'high';
				fastSamples = 0;
			}
		}
	}

	function schedule() {
		clearTimeout(timer);
		if (active || !quality) return;
		let next: Demand | undefined;
		for (const demand of demands.values()) {
			if (!next || demand.due < next.due) next = demand;
		}
		if (next) {
			const demand = next;
			timer = setTimeout(
				() => void pull(demand),
				Math.max(0, Math.max(demand.due, nextRequestAt) - performance.now())
			);
		}
	}

	async function pull(demand: Demand) {
		const requested = quality;
		if (!requested || active || demands.get(demand.source) !== demand) return;
		const abort = new AbortController();
		const timeout = AbortSignal.timeout(REQUEST_TIMEOUT_MS);
		const signal = AbortSignal.any([abort.signal, timeout]);
		active = { demand, abort };
		const began = performance.now();
		nextRequestAt = began + REQUEST_INTERVAL[requested];
		const probing = mode === 'auto' && requested === 'low' && began >= nextLinkProbe;
		if (probing) nextLinkProbe = began + 400;
		let minimumWait = FRAME_INTERVAL[requested];
		try {
			const frame = await demand.request(
				{ source: demand.source, quality: requested, after: probing ? null : demand.after },
				signal
			);
			if (!frame || signal.aborted) return;
			observe(requested, performance.now() - began, frame.image.size);
			const image = await createImageBitmap(frame.image);
			try {
				if (!abort.signal.aborted) {
					for (const show of demand.listeners) show(image, frame.extent);
					demand.after = frame.tag;
				}
			} finally {
				image.close();
			}
		} catch {
			if (!abort.signal.aborted) {
				if (timeout.aborted) observe(requested, REQUEST_TIMEOUT_MS, 0);
				minimumWait = 500;
			}
		} finally {
			if (!abort.signal.aborted) demand.due = began + minimumWait;
			active = null;
			schedule();
		}
	}

	return {
		get mode() {
			return mode;
		},
		get quality() {
			return quality;
		},
		setMode(next: BrowserPreviewMode) {
			if (next === mode) return;
			mode = next;
			if (next === 'auto') {
				automaticQuality = 'high';
				highBytes = 0;
				fastSamples = 0;
			}
			active?.abort.abort();
			nextRequestAt = 0;
			for (const demand of demands.values()) demand.due = 0;
			schedule();
		},
		register(source: string, request: UiBrowserPreview, show: RenderFrame) {
			const demand = demands.get(source) ?? {
				source,
				request,
				listeners: new Set(),
				after: null,
				due: 0
			};
			demand.listeners.add(show);
			demand.after = null;
			demand.due = 0;
			demands.set(source, demand);
			schedule();
			return () => {
				demand.listeners.delete(show);
				if (demand.listeners.size) return;
				demands.delete(source);
				if (active?.demand === demand) active.abort.abort();
				schedule();
			};
		}
	};
}

type BrowserPreviewSession = ReturnType<typeof browserPreviewSession>;

export const provideBrowserPreviews = () => setContext(CONTEXT, browserPreviewSession());
export const useBrowserPreviews = () => getContext<BrowserPreviewSession>(CONTEXT);
