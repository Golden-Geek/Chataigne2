import type { PreviewPointer } from './generated/PreviewPointer';

const projected = new WeakMap<Event, { capture?: Element }>();

/** Native projection retains OS capture on the viewport. A control records its logical
 * capture target here, including when its queued down/up finish after the OS pointer ended. */
export function capturePreviewPointer(element: Element, event: PointerEvent) {
	const context = projected.get(event);
	if (context) context.capture = element;
	else element.setPointerCapture(event.pointerId);
}

/** Native admission/hit testing stays serialized. An owned drag projects moves locally in
 * its accepted coordinate basis; geometry changes cancel it instead of retargeting capture. */
export function observeNativeInput(
	element: HTMLElement,
	query: (x: number, y: number) => Promise<PreviewPointer | null>,
	enabled: () => boolean
) {
	const root = element.closest<HTMLElement>('[data-native-preview-backdrop]') ?? element;
	let disposed = false,
		running = false,
		epoch = 0;
	let latest: PointerEvent | null = null;
	let terminal: PointerEvent | null = null;
	let active: {
		down: PointerEvent;
		target: Element | null;
		hit: string | null;
		capture: Element | null;
		started: boolean;
		projection: { native: PreviewPointer['rect']; dom: PreviewPointer['rect']; dpr: number } | null;
	} | null = null;
	let hovered: Element | null = null;
	let last: PreviewPointer | null = null;
	let lastEvent: PointerEvent | null = null;
	const excluded = (event: Event) =>
		event.target instanceof Element &&
		!!event.target.closest('[data-native-preview-input-exclude]');
	const owns = (event: Event) =>
		!disposed && enabled() && !projected.has(event) && !excluded(event);
	const bounds = () => {
		const { x, y, width, height } = element.getBoundingClientRect();
		return { x, y, width, height };
	};
	function sameBounds(left: PreviewPointer['rect'], right: PreviewPointer['rect']) {
		return (
			left.x === right.x &&
			left.y === right.y &&
			left.width === right.width &&
			left.height === right.height
		);
	}
	function validProjection(rect: PreviewPointer['rect']) {
		return Object.values(rect).every(Number.isFinite) && rect.width > 0 && rect.height > 0;
	}
	function capturedSample(event: PointerEvent, hit: string | null): PreviewPointer | null {
		const projection = active?.projection;
		if (!projection || projection.dpr !== devicePixelRatio || !sameBounds(projection.dom, bounds()))
			return null;
		const rect = projection.native;
		const point: [number, number] = [
			(event.clientX - rect.x) / rect.width,
			(event.clientY - rect.y) / rect.height
		];
		return point.every(Number.isFinite) ? { rect, point, hit } : null;
	}
	function target(hit: string | null) {
		if (hit === null) return root;
		const found = element.querySelector(`[data-native-hit-id="${CSS.escape(hit)}"]`);
		return found && !found.matches(':disabled, [aria-disabled="true"]') ? found : null;
	}
	function dispatch(
		type: string,
		destination: Element,
		original: PointerEvent,
		sample: PreviewPointer,
		relatedTarget: Element | null = null
	) {
		const rect = element.getBoundingClientRect();
		const event = new PointerEvent(type, {
			bubbles: type !== 'pointerenter' && type !== 'pointerleave',
			cancelable: true,
			clientX: rect.x + sample.point[0] * rect.width,
			clientY: rect.y + sample.point[1] * rect.height,
			pointerId: original.pointerId,
			pointerType: original.pointerType,
			isPrimary: original.isPrimary,
			button: original.button,
			buttons: original.buttons,
			detail: type === 'click' ? 1 : 0,
			altKey: original.altKey,
			ctrlKey: original.ctrlKey,
			shiftKey: original.shiftKey,
			metaKey: original.metaKey,
			relatedTarget
		});
		const context: { capture?: Element } = {};
		projected.set(event, context);
		destination.dispatchEvent(event);
		return context.capture ?? null;
	}
	function hover(next: Element | null, event: PointerEvent, sample: PreviewPointer) {
		if (next === root) next = null;
		if (hovered === next) return;
		if (hovered?.isConnected) dispatch('pointerleave', hovered, event, sample, next);
		if (next?.isConnected) dispatch('pointerenter', next, event, sample, hovered);
		hovered = next;
	}
	function release() {
		if (active && root.hasPointerCapture(active.down.pointerId))
			root.releasePointerCapture(active.down.pointerId);
		active = null;
	}
	function cancel() {
		epoch++;
		latest = terminal = null;
		if (active?.started && last) {
			const destination = active.capture ?? active.target;
			if (destination?.isConnected) dispatch('pointercancel', destination, active.down, last);
		}
		if (active)
			dispatch(
				'pointercancel',
				root,
				active.down,
				last ?? { point: [0, 0], rect: element.getBoundingClientRect(), hit: null }
			);
		release();
		if (last && lastEvent) hover(null, lastEvent, last);
	}
	async function drain() {
		if (running) return;
		running = true;
		try {
			while (!disposed && (latest || terminal || (active && !active.started))) {
				const beginning = active && !active.started ? active : null;
				const event = beginning?.down ?? latest ?? terminal!;
				if (!beginning) {
					if (event === latest) latest = null;
					else terminal = null;
				}
				const before = epoch;
				const dom = bounds();
				// Only a control's explicit logical capture admits local movement. Hover, down
				// and terminal hit testing still use the accepted native frame.
				const local = !beginning && event.type === 'pointermove' && active?.capture;
				let sample: PreviewPointer | null;
				if (local) sample = capturedSample(event, active!.hit);
				else {
					try {
						sample = await query(event.clientX, event.clientY);
					} catch {
						if (!disposed && before === epoch) cancel();
						continue;
					}
				}
				if (disposed || before !== epoch) continue;
				if (!sample || !enabled() || !sameBounds(dom, bounds())) {
					cancel();
					continue;
				}
				if (active?.capture && !beginning && !local) {
					sample = capturedSample(event, sample.hit);
					if (!sample) {
						cancel();
						continue;
					}
				}
				last = sample;
				lastEvent = event;
				if (beginning) {
					beginning.started = true;
					beginning.hit = sample.hit;
					beginning.target = target(sample.hit);
					if (!validProjection(sample.rect) || !validProjection(dom)) {
						cancel();
						continue;
					}
					beginning.projection = { native: { ...sample.rect }, dom, dpr: devicePixelRatio };
					if (beginning.target)
						beginning.capture = dispatch('pointerdown', beginning.target, event, sample);
				} else if (active) {
					const destination = active.capture ?? active.target;
					if (!destination?.isConnected) {
						cancel();
						continue;
					}
					dispatch(event.type, destination, event, sample);
					if (event.type === 'pointerup') {
						if (sample.hit === active.hit && active.target?.isConnected)
							dispatch('click', active.target, event, sample);
						release();
					}
				} else hover(target(sample.hit), event, sample);
			}
		} finally {
			running = false;
		}
	}
	function down(event: PointerEvent) {
		if (!owns(event) || event.defaultPrevented || event.button !== 0 || active) return;
		event.preventDefault();
		event.stopPropagation();
		epoch++;
		latest = terminal = null;
		active = {
			down: event,
			target: null,
			hit: null,
			capture: null,
			started: false,
			projection: null
		};
		root.setPointerCapture(event.pointerId);
		void drain();
	}
	function move(event: PointerEvent) {
		// Navigation owns its captured Space/middle-button sequence.
		if (!active && event.buttons !== 0) return;
		if (
			!owns(event) ||
			event.defaultPrevented ||
			(active && event.pointerId !== active.down.pointerId)
		)
			return;
		event.stopPropagation();
		if (terminal) return;
		latest = event;
		void drain();
	}
	function up(event: PointerEvent) {
		if (!owns(event) || !active || event.pointerId !== active.down.pointerId) return;
		event.preventDefault();
		event.stopPropagation();
		terminal = event;
		void drain();
	}
	function cancelled(event: PointerEvent) {
		if (!owns(event) || !active || event.pointerId !== active.down.pointerId) return;
		event.stopPropagation();
		cancel();
	}
	function blockHover(event: Event) {
		if (!owns(event)) return;
		if (
			event instanceof PointerEvent &&
			event.type === 'pointerleave' &&
			event.target === root &&
			!active &&
			last
		) {
			epoch++;
			latest = null;
			hover(null, event, last);
		}
		event.stopPropagation();
	}
	function click(event: MouseEvent) {
		if (owns(event) && event.detail !== 0) {
			event.preventDefault();
			event.stopPropagation();
		}
	}
	function key(event: KeyboardEvent) {
		if (event.key === 'Escape') cancel();
	}
	function geometryChanged() {
		const projection = active?.projection;
		if (
			projection &&
			(projection.dpr !== devicePixelRatio || !sameBounds(projection.dom, bounds()))
		)
			cancel();
	}
	const resize = new ResizeObserver(geometryChanged);
	resize.observe(element);
	root.addEventListener('pointerdown', down, true);
	root.addEventListener('pointermove', move, true);
	root.addEventListener('pointerup', up, true);
	root.addEventListener('pointercancel', cancelled, true);
	root.addEventListener('click', click, true);
	for (const name of ['pointerenter', 'pointerleave', 'pointerover', 'pointerout'])
		root.addEventListener(name, blockHover, true);
	window.addEventListener('blur', cancel);
	window.addEventListener('keydown', key);
	window.addEventListener('resize', geometryChanged);
	window.addEventListener('scroll', geometryChanged, true);
	return () => {
		cancel();
		disposed = true;
		root.removeEventListener('pointerdown', down, true);
		root.removeEventListener('pointermove', move, true);
		root.removeEventListener('pointerup', up, true);
		root.removeEventListener('pointercancel', cancelled, true);
		root.removeEventListener('click', click, true);
		for (const name of ['pointerenter', 'pointerleave', 'pointerover', 'pointerout'])
			root.removeEventListener(name, blockHover, true);
		window.removeEventListener('blur', cancel);
		window.removeEventListener('keydown', key);
		window.removeEventListener('resize', geometryChanged);
		window.removeEventListener('scroll', geometryChanged, true);
		resize.disconnect();
	};
}
