export interface AudioInspectorIntentOutcome {
	readonly accepted: boolean;
	readonly ignored: boolean;
	readonly message: string | null;
}

export class AudioInspectorInteractionCoordinator {
	private revision = 0;
	private disposed = false;

	async submit(
		action: () => Promise<boolean>,
		rollback: () => void
	): Promise<AudioInspectorIntentOutcome> {
		try {
			const accepted = await action();
			if (this.disposed) {
				return { accepted, ignored: true, message: null };
			}
			if (!accepted) {
				rollback();
				return {
					accepted: false,
					ignored: false,
					message: 'The audio setting was rejected and has been restored.'
				};
			}
			return { accepted: true, ignored: false, message: null };
		} catch (error) {
			if (this.disposed) {
				return { accepted: false, ignored: true, message: null };
			}
			rollback();
			return {
				accepted: false,
				ignored: false,
				message: error instanceof Error ? error.message : 'The audio setting could not be changed.'
			};
		}
	}

	async refresh(
		action: () => Promise<void>,
		onSettled: (message: string | null) => void
	): Promise<void> {
		const revision = ++this.revision;
		let message: string | null = null;
		try {
			await action();
		} catch (error) {
			message = error instanceof Error ? error.message : 'Audio device discovery failed.';
		}
		if (!this.disposed && revision === this.revision) {
			onSettled(message);
		}
	}

	dispose(): void {
		this.disposed = true;
		this.revision += 1;
	}
}
