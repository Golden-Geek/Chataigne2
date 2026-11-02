export interface UiEditSession {
	readonly active: boolean;
	begin(): Promise<void>;
	end(): Promise<void>;
}

interface EditHost {
	readonly hasActiveEditSession: boolean;
	readonly editSessionEpoch: number;
	readonly snapshotGeneration: number;
}

/** Serialize each owner's begin/end acknowledgement without closing another host or epoch. */
export function createEditSession<Host extends EditHost>(context: {
	host(): Host | null;
	begin(host: Host): Promise<boolean>;
	end(host: Host): Promise<unknown>;
}): UiEditSession {
	let owner: { host: Host; epoch: number; generation: number } | null = null;
	let starting: Promise<void> | null = null;
	let ending: Promise<void> | null = null;
	let reopening: {
		host: Host | null;
		generation: number | undefined;
		cancelled: boolean;
		started: boolean;
		promise: Promise<void>;
	} | null = null;
	const current = () =>
		owner !== null &&
		context.host() === owner.host &&
		owner.host.snapshotGeneration === owner.generation &&
		owner.host.hasActiveEditSession &&
		owner.host.editSessionEpoch === owner.epoch;
	const begin = (): Promise<void> => {
		if (ending) {
			if (reopening && !reopening.cancelled) return reopening.promise;
			const next = {
				host: context.host(),
				generation: context.host()?.snapshotGeneration,
				cancelled: false,
				started: false,
				promise: Promise.resolve()
			};
			next.promise = ending.then(() => {
				if (
					next.cancelled ||
					context.host() !== next.host ||
					next.host?.snapshotGeneration !== next.generation
				)
					return;
				next.started = true;
				return begin();
			});
			reopening = next;
			const clear = () => {
				if (reopening === next) reopening = null;
			};
			void next.promise.then(clear, clear);
			return next.promise;
		}
		if (starting) return starting;
		if (current()) return Promise.resolve();
		owner = null;
		const host = context.host();
		if (!host || host.hasActiveEditSession) return Promise.resolve();
		const generation = host.snapshotGeneration;
		const task = (async () => {
			if (await context.begin(host)) owner = { host, epoch: host.editSessionEpoch, generation };
		})();
		starting = task;
		const clear = () => {
			if (starting === task) starting = null;
		};
		void task.then(clear, clear);
		return task;
	};
	const end = (): Promise<void> => {
		if (reopening) {
			const pending = reopening;
			pending.cancelled = true;
			return pending.promise.then(() => (pending.started ? end() : undefined));
		}
		if (ending) return ending;
		const task = (async () => {
			await starting;
			const previous = owner,
				valid = current();
			owner = null;
			if (previous && valid) await context.end(previous.host);
		})();
		ending = task;
		const clear = () => {
			if (ending === task) ending = null;
		};
		void task.then(clear, clear);
		return task;
	};
	return {
		get active() {
			return current();
		},
		begin,
		end
	};
}
