import type { MatrixMix } from '../preview/generated/MatrixMix';
import type { MatrixMixInterval } from '../preview/generated/MatrixMixInterval';
import type { MatrixPlaybackMix } from '../preview/generated/MatrixPlaybackMix';

type Envelope = { from: number; to: number; interval: MatrixMixInterval | null };
export type PlaybackLines = { incoming: Envelope[]; outgoing: Envelope[] };

function byColumn(mix: MatrixMix, includeIndependent: boolean) {
	const columns = new Map<string, PlaybackLines>();
	for (const { column, from, to, individual } of mix.weights) {
		if (individual && !includeIndependent) continue;
		let lines = columns.get(column);
		if (!lines) {
			lines = { incoming: [], outgoing: [] };
			columns.set(column, lines);
		}
		const side = to > 0 ? lines.incoming : lines.outgoing;
		if (side[0]) {
			side[0].from += from;
			side[0].to += to;
		} else side.push({ from, to, interval: mix.interval });
	}
	return columns;
}

/** Use the largest weight across layers launched by a Column. Independent Cell launches
 * get their own Cell lines, and may retain outgoing contributions from a Column launch. */
export function playbackLines(mix: MatrixPlaybackMix | null | undefined) {
	const columns = new Map<string, PlaybackLines>();
	const layers = new Map<string, Map<string, PlaybackLines>>();
	const base = mix ? byColumn(mix.base, true) : new Map<string, PlaybackLines>();
	let until = 0;
	if (mix) {
		const heading = new Map<string, Map<string, Envelope>>();
		for (const local of [mix.base, ...mix.layers.map((entry) => entry.mix)]) {
			const interval = local.interval;
			if (interval) until = Math.max(until, interval.start_ms + interval.duration_ms);
			if (local === mix.base && !mix.base_in_use) continue;
			for (const [id, lines] of byColumn(local, false)) {
				let segments = heading.get(id);
				if (!segments) {
					segments = new Map();
					heading.set(id, segments);
				}
				for (const line of [...lines.incoming, ...lines.outgoing]) {
					const key = `${line.to}:${interval?.start_ms}:${interval?.duration_ms}:${interval?.curve}`;
					const previous = segments.get(key);
					if (!previous || line.from > previous.from) segments.set(key, line);
				}
			}
		}
		for (const [id, segments] of heading) {
			const lines: PlaybackLines = { incoming: [], outgoing: [] };
			for (const line of segments.values())
				(line.to > 0 ? lines.incoming : lines.outgoing).push(line);
			columns.set(id, lines);
		}
		for (const entry of mix.layers) layers.set(entry.layer, byColumn(entry.mix, true));
	}
	return {
		columns,
		until,
		cell: (layer: string, column: string) => (layers.get(layer) ?? base).get(column)
	};
}

export function lineWeight(lines: Envelope[], now: number) {
	let weight = 0;
	for (const { from, to, interval } of lines) {
		const elapsed = interval
			? Math.min(1, Math.max(0, (now - interval.start_ms) / interval.duration_ms))
			: 1;
		const fraction = interval?.curve === 'Smooth' ? elapsed * elapsed * (3 - 2 * elapsed) : elapsed;
		weight = Math.max(weight, from + (to - from) * fraction);
	}
	return Math.min(1, weight);
}
