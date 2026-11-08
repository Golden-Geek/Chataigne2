import type { VideoSourceStatus } from './generated/VideoSourceStatus';

export const createVideoSourceState = () => ({
	status: null as VideoSourceStatus | null,
	error: ''
});
type State = ReturnType<typeof createVideoSourceState>;
type Later = (callback: () => void) => () => void;
const later: Later = (callback) => {
	const timer = setTimeout(callback, 1000);
	return () => clearTimeout(timer);
};

/** One selected-source read at a time; no decoder work or authored activity in this owner. */
export class VideoSourceReader {
	private epoch = 0;
	private cancel: (() => void) | undefined;
	private state: State;
	private query: () => Promise<VideoSourceStatus | null>;
	private live: () => boolean;
	private schedule: Later;
	constructor(
		state: State,
		query: () => Promise<VideoSourceStatus | null>,
		live: () => boolean,
		schedule: Later = later
	) {
		this.state = state;
		this.query = query;
		this.live = live;
		this.schedule = schedule;
	}
	start() {
		this.stop();
		const epoch = this.epoch;
		void this.poll(epoch);
		return () => {
			if (epoch === this.epoch) this.stop();
		};
	}
	private async poll(epoch: number) {
		if (epoch !== this.epoch) return;
		this.cancel = undefined;
		const current = () => epoch === this.epoch && this.live();
		if (!current()) {
			this.state.status = null;
			this.state.error = '';
			return;
		}
		try {
			const status = await this.query();
			if (current() && status) {
				this.state.status = status;
				this.state.error = '';
			}
		} catch (error) {
			if (current()) {
				this.state.status = null;
				this.state.error = String(error);
			}
		} finally {
			if (current())
				this.cancel = this.schedule(() => {
					void this.poll(epoch);
				});
			else if (epoch === this.epoch) {
				this.state.status = null;
				this.state.error = '';
			}
		}
	}
	stop() {
		this.epoch++;
		this.cancel?.();
		this.cancel = undefined;
		this.state.status = null;
		this.state.error = '';
	}
}
