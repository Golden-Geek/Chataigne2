import type { SpatialEndpoint } from './types';

/** Unrelated value publications must not rebuild map geometry or navigation. */
export class SpatializerEndpointCache {
	#epoch: unknown;
	#byKey = new Map<string, SpatialEndpoint>();
	#ordered: SpatialEndpoint[] = [];

	reconcile(candidates: SpatialEndpoint[], epoch: unknown): SpatialEndpoint[] {
		if (epoch !== this.#epoch) {
			this.#byKey.clear();
			this.#ordered = [];
			this.#epoch = epoch;
		}
		const next = candidates.map((candidate) => {
			const previous = this.#byKey.get(candidate.key);
			// All presentation values come from these immutable DTOs, except
			// palette color (which also depends on the endpoint's list order).
			return previous &&
				previous.node === candidate.node &&
				previous.positionParam === candidate.positionParam &&
				previous.radiusParam === candidate.radiusParam &&
				previous.freezeRadiusParam === candidate.freezeRadiusParam &&
				previous.kind === candidate.kind &&
				previous.color === candidate.color
				? previous
				: candidate;
		});
		this.#byKey = new Map(next.map((endpoint) => [endpoint.key, endpoint]));
		if (
			next.length !== this.#ordered.length ||
			next.some((endpoint, i) => endpoint !== this.#ordered[i])
		)
			this.#ordered = next;
		return this.#ordered;
	}
}
