interface PendingRead<T> {
	resolve(value: T): void;
	reject(error: Error): void;
	timer: ReturnType<typeof setTimeout>;
}

export function completeWsRead<T>(pending: PendingRead<T>, decode: () => T): void {
	clearTimeout(pending.timer);
	try {
		pending.resolve(decode());
	} catch (error) {
		pending.reject(error instanceof Error ? error : new Error(String(error)));
	}
}

/** Read requests share timeout/retirement ownership; mutation receipt handling stays separate. */
export function requestWsRead<T>(context: {
	pending: Map<string, PendingRead<T>>;
	id: string;
	kind: string;
	timeout: number;
	send(): Promise<unknown>;
}): Promise<T> {
	return new Promise<T>((resolve, reject) => {
		const timer = setTimeout(() => {
			context.pending.delete(context.id);
			reject(new Error(`${context.kind} timeout (${context.id})`));
		}, context.timeout);
		context.pending.set(context.id, { resolve, reject, timer });
		void Promise.resolve()
			.then(context.send)
			.catch((error) => {
				clearTimeout(timer);
				context.pending.delete(context.id);
				reject(error);
			});
	});
}
