import type { VideoRuntimeStatus } from '../preview/generated/VideoRuntimeStatus';
import type { VideoStatusTransport } from './transport';

export const createVideoStatusState = () => ({
	status: null as VideoRuntimeStatus | null,
	connection: 'unavailable' as 'unavailable' | 'waiting' | 'connected' | 'error',
	error: ''
});
type State = ReturnType<typeof createVideoStatusState>;

/** Subscribe-before-read, sequenced snapshots, one recovery timer and explicit disposal fencing. */
export class VideoStatusController {
	private transport: VideoStatusTransport;
	private state: State;
	private epoch = 0;
	private lifetime = 0;
	private stopped = true;
	private unsubscribe: (() => void) | undefined;
	private retry: ReturnType<typeof setTimeout> | undefined;
	constructor(transport: VideoStatusTransport, state: State) {
		this.transport = transport;
		this.state = state;
	}
	connect() {
		this.stop();
		this.stopped = false;
		const lifetime = this.lifetime;
		if (this.transport.available()) void this.attempt();
		return () => {
			if (lifetime === this.lifetime) this.stop();
		};
	}
	private async attempt() {
		if (this.stopped) return;
		const epoch = ++this.epoch;
		let sequence = -1;
		this.state.connection = 'waiting';
		const accept = (status: VideoRuntimeStatus) => {
			if (this.stopped || epoch !== this.epoch || status.sequence <= sequence) return;
			sequence = status.sequence;
			this.state.status = status;
			this.state.connection = 'connected';
			this.state.error = '';
			clearTimeout(this.retry);
			this.retry = setTimeout(
				() => this.recover(epoch, new Error('Video runtime observation stream expired')),
				3000
			);
		};
		try {
			const dispose = await this.transport.subscribe(accept);
			if (this.stopped || epoch !== this.epoch) {
				dispose();
				return;
			}
			this.unsubscribe = dispose;
			accept(await this.transport.read());
		} catch (error) {
			this.recover(epoch, error);
		}
	}
	private recover(epoch: number, error: unknown) {
		if (this.stopped || epoch !== this.epoch) return;
		this.epoch++;
		this.unsubscribe?.();
		this.unsubscribe = undefined;
		this.state.status = null;
		this.state.connection = 'error';
		this.state.error = String(error);
		clearTimeout(this.retry);
		this.retry = setTimeout(() => {
			this.retry = undefined;
			void this.attempt();
		}, 1000);
	}
	stop() {
		this.stopped = true;
		this.epoch++;
		this.lifetime++;
		clearTimeout(this.retry);
		this.retry = undefined;
		this.unsubscribe?.();
		this.unsubscribe = undefined;
		this.state.status = null;
		this.state.connection = 'unavailable';
		this.state.error = '';
	}
}
