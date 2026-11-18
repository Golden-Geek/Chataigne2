import type { Point } from './generated/Point';
import type { ScreenHandleCandidate } from './generated/ScreenHandleCandidate';

/** Viewport proximity only. The backend supplies eligibility and owns link semantics. */
export function nearestHandle(
	point: Point,
	corner: number,
	candidates: readonly ScreenHandleCandidate[],
	viewport: { width: number; height: number },
	threshold: number
): ScreenHandleCandidate | null {
	if (!(viewport.width > 0 && viewport.height > 0 && threshold > 0)) return null;
	let best: ScreenHandleCandidate | null = null;
	let distance = threshold;
	const key = (candidate: ScreenHandleCandidate) =>
		`${candidate.member.reference.screen}:${candidate.member.reference.surface}:${candidate.member.corner}`;
	for (const candidate of candidates) {
		if (!candidate.linkable[corner]) continue;
		const next = Math.hypot(
			(candidate.point.x - point.x) * viewport.width,
			(candidate.point.y - point.y) * viewport.height
		);
		if (next < distance || (next === distance && (!best || key(candidate) < key(best)))) {
			best = candidate;
			distance = next;
		}
	}
	return best;
}
