/** One active read and one latest replacement; expired contexts never dispatch queued work. */
export class ReadQueue {
	private revision = 0;
	private active = false;
	private next: { id: number; run(): Promise<void> } | null = null;
	invalidate() {
		++this.revision;
		this.next = null;
	}
	request<T>(
		current: () => boolean,
		read: () => Promise<T>,
		accept: (value: T) => void,
		reject: (error: unknown) => void,
		finish: () => void
	) {
		const id = ++this.revision;
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
