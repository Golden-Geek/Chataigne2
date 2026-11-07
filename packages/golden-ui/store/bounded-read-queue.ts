/** One active read and one latest replacement; expired contexts never dispatch queued work. */
export class ReadQueue {
	private revision = 0;
	private active = false;
	private next: { id: number; run(): Promise<void> } | null = null;
	private context: readonly unknown[] | null = null;
	invalidate() {
		++this.revision;
		this.next = null;
		this.context = null;
	}
	request<T>(
		current: () => boolean,
		read: () => Promise<T>,
		accept: (value: T) => void,
		reject: (error: unknown) => void,
		finish: () => void
	) {
		this.context = null;
		const id = ++this.revision;
		this.enqueue(id, current, read, accept, reject, finish);
	}
	/** Publish sequential observations of one context even while newer observations queue. */
	refresh<T>(
		context: readonly unknown[],
		current: () => boolean,
		read: () => Promise<T>,
		accept: (value: T) => void,
		reject: (error: unknown) => void,
		finish: () => void
	) {
		if (
			!this.context ||
			this.context.length !== context.length ||
			context.some((value, index) => !Object.is(value, this.context![index]))
		) {
			this.invalidate();
			this.context = [...context];
		}
		this.enqueue(this.revision, current, read, accept, reject, finish);
	}
	private enqueue<T>(
		id: number,
		current: () => boolean,
		read: () => Promise<T>,
		accept: (value: T) => void,
		reject: (error: unknown) => void,
		finish: () => void
	) {
		this.next = {
			id,
			run: async () => {
				try {
					if (!current()) return;
					const value = await read();
					if (id === this.revision) accept(value);
				} catch (error) {
					if (id === this.revision) reject(error);
				} finally {
					if (id === this.revision) finish();
				}
			}
		};
		void this.drain();
	}
	private async drain() {
		if (this.active) return;
		this.active = true;
		try {
			while (this.next) {
				const next = this.next;
				this.next = null;
				await next.run();
			}
		} finally {
			this.active = false;
		}
	}
}
