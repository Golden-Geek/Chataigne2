import { VIEWPORT_FEEDBACK_MS } from 'golden_ui/components/viewport/feedback.ts';
import type { OverlayPrimitive } from './generated/OverlayPrimitive';

type Color = OverlayPrimitive['fill'];
interface Entry {
	target: OverlayPrimitive;
	fill: Color;
	stroke: Color;
	started: number;
}
const same = (a: Color, b: Color) => a.every((value, index) => value === b[index]);
const blend = (from: Color, to: Color, amount: number): Color =>
	from.map((value, index) => value + (to[index] - value) * amount) as Color;
const key = (primitive: OverlayPrimitive, index: number) =>
	`${primitive.shape.kind}:${primitive.hit === null ? `decoration:${index}` : `hit:${primitive.hit}`}`;

/** One finite paint animation per preview. Target geometry and hit IDs always publish immediately. */
export function animateOverlayPaint(view: Window, publish: (paint: OverlayPrimitive[]) => void) {
	const preference = view.matchMedia('(prefers-reduced-motion: reduce)');
	let entries = new Map<string, Entry>();
	let frame: number | null = null;
	let disposed = false;
	let initialized = false;
	function amount(entry: Entry, now: number) {
		return preference.matches
			? 1
			: Math.min(1, Math.max(0, (now - entry.started) / VIEWPORT_FEEDBACK_MS));
	}
	function color(entry: Entry, now: number) {
		const progress = amount(entry, now);
		if (progress >= 1) return { fill: entry.target.fill, stroke: entry.target.stroke };
		return {
			fill: blend(entry.fill, entry.target.fill, progress),
			stroke: blend(entry.stroke, entry.target.stroke, progress)
		};
	}
	function cancel() {
		if (frame !== null) view.cancelAnimationFrame(frame);
		frame = null;
	}
	function paint(now: number) {
		if (disposed) return;
		let active = false;
		publish(
			[...entries.values()].map((entry) => {
				active ||=
					amount(entry, now) < 1 &&
					(!same(entry.fill, entry.target.fill) || !same(entry.stroke, entry.target.stroke));
				return amount(entry, now) >= 1 ? entry.target : { ...entry.target, ...color(entry, now) };
			})
		);
		if (active && frame === null)
			frame = view.requestAnimationFrame((time) => {
				frame = null;
				paint(time);
			});
		if (!active) cancel();
	}
	function settle() {
		cancel();
		for (const entry of entries.values()) entry.started = -Infinity;
		paint(view.performance.now());
	}
	preference.addEventListener('change', settle);
	return {
		update(targets: readonly OverlayPrimitive[]) {
			if (disposed) return;
			const now = view.performance.now();
			const next = new Map<string, Entry>();
			targets.forEach((target, index) => {
				const identity = key(target, index),
					previous = entries.get(identity);
				if (
					previous &&
					same(previous.target.fill, target.fill) &&
					same(previous.target.stroke, target.stroke)
				) {
					next.set(identity, { ...previous, target });
				} else {
					const from = previous
						? color(previous, now)
						: initialized
							? {
									fill: [...target.fill.slice(0, 3), 0] as Color,
									stroke: [...target.stroke.slice(0, 3), 0] as Color
								}
							: { fill: target.fill, stroke: target.stroke };
					next.set(identity, { target, ...from, started: now });
				}
			});
			entries = next;
			initialized = true;
			paint(now);
		},
		destroy() {
			disposed = true;
			cancel();
			entries.clear();
			preference.removeEventListener('change', settle);
		}
	};
}
