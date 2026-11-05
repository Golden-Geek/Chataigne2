import { PREVIEW_PROTOCOL_VERSION } from './generated/constants';
import type { PreviewView } from './generated/PreviewView';
import type { PreviewRequest } from './generated/PreviewRequest';
import type { PreviewResponse } from './generated/PreviewResponse';
import type { PreviewTransport } from './transport';
import type { OverlayPrimitive } from './generated/OverlayPrimitive';
import type { PreviewPointer } from './generated/PreviewPointer';
import { RenderOpportunity } from './render-opportunity';

export type PreviewGeometry = Pick<PreviewView, 'rect' | 'clip' | 'visible'> & {
	overlays?: OverlayPrimitive[];
	overlay_capacity?: number;
	tile_groups?: PreviewView['tile_groups'];
};
export interface PreviewSessionState {
	availability: 'idle' | 'connecting' | 'available' | 'unavailable' | 'disposed';
	session: string | null;
	acknowledgedRevision: number;
	visibleViews: number;
	installed: boolean;
	/** Visible registrations with an installed frame to retain while source/layout replacements
	 * are pending. This is distinct from confirmation of the latest layout installation. */
	presentedViews: readonly string[];
	error: string | null;
}
export interface PreviewRegistration {
	readonly id: string;
	update(source: string): void;
	invalidate(): void;
	unregister(): void;
	pointer(x: number, y: number): Promise<PreviewPointer | null>;
}
export interface PreviewSession {
	readonly state: PreviewSessionState;
	connect(): Promise<boolean>;
	reconnect(): Promise<boolean>;
	register(source: string, measure: () => PreviewGeometry): PreviewRegistration;
	invalidate(): void;
	subscribe(listener: (state: PreviewSessionState) => void): () => void;
	whenSettled(): Promise<void>;
	dispose(): Promise<void>;
}

interface Registration {
	id: string;
	source: string;
	measure: () => PreviewGeometry;
}
const MAX_VIEWS = 64;
const HEARTBEAT_MS = 1000;
const newId = (): string => globalThis.crypto.randomUUID();

/** One window's presentation owner. The single pump serializes Connect/Layout/Disconnect;
 * layout changes retain one latest desired snapshot while an IPC is in flight. */
export function createPreviewSession(transport: PreviewTransport): PreviewSession {
	let state: PreviewSessionState = {
		availability: 'idle',
		session: null,
		acknowledgedRevision: 0,
		visibleViews: 0,
		installed: false,
		presentedViews: [],
		error: null
	};
	let generation = 0;
	let revision = 0;
	let client = '';
	let connectWanted = false;
	let dirty = false;
	let running = false;
	let disposed = false;
	let closing = false;
	let frame: ReturnType<typeof setTimeout> | number | null = null;
	let heartbeat: ReturnType<typeof setTimeout> | null = null;
	let statusTimer: ReturnType<typeof setTimeout> | null = null;
	let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
	let retryDelay = 1000;
	let statusWanted = false;
	let statusAttempts = 0;
	let lastGeometry = '';
	let lastViews: PreviewView[] = [];
	let presentedLayout: PreviewView[] = [];
	let pendingApplied: Extract<PreviewResponse, { kind: 'Applied' }> | null = null;
	let maximumViews = MAX_VIEWS;
	const registrations = new Map<string, Registration>();
	const rendering = new RenderOpportunity();
	const listeners = new Set<(state: PreviewSessionState) => void>();
	const cleanupSessions = new Set<string>();
	const idleWaiters = new Set<() => void>();
	let connection: Promise<boolean> = Promise.resolve(false);
	let resolveConnection: ((available: boolean) => void) | undefined;

	function update(next: Partial<PreviewSessionState>) {
		state = { ...state, ...next };
		for (const listener of listeners) listener(state);
	}
	function retainPresentation(views: PreviewView[]) {
		const presented = new Set(
			presentedLayout.filter((view) => view.visible).map((view) => view.id)
		);
		return views.filter((view) => view.visible && presented.has(view.id)).map((view) => view.id);
	}
	function stopHeartbeat() {
		if (heartbeat !== null) clearTimeout(heartbeat);
		heartbeat = null;
	}
	function startHeartbeat() {
		stopHeartbeat();
		if (disposed || state.availability !== 'available') return;
		heartbeat = setTimeout(() => {
			heartbeat = null;
			schedule(false);
			startHeartbeat();
		}, HEARTBEAT_MS);
	}
	function stopStatus() {
		if (statusTimer !== null) clearTimeout(statusTimer);
		statusTimer = null;
		statusWanted = false;
	}
	function stopReconnect() {
		if (reconnectTimer !== null) clearTimeout(reconnectTimer);
		reconnectTimer = null;
	}
	function scheduleStatus() {
		if ((disposed && !closing) || statusTimer !== null || state.installed) return;
		if (statusAttempts >= 10) {
			if (closing) fail('Native preview clear could not be confirmed');
			return;
		}
		statusTimer = setTimeout(() => {
			statusTimer = null;
			statusWanted = true;
			void pump();
		}, 100);
	}
	function cancelFrame() {
		if (frame === null) return;
		if (typeof cancelAnimationFrame === 'function') cancelAnimationFrame(frame as number);
		else clearTimeout(frame);
		frame = null;
	}
	function settle() {
		if (
			running ||
			frame !== null ||
			statusTimer !== null ||
			statusWanted ||
			connectWanted ||
			cleanupSessions.size ||
			(dirty && state.availability === 'available')
		)
			return;
		for (const waiter of idleWaiters) waiter();
		idleWaiters.clear();
	}
	function fail(error: string) {
		pendingApplied = null;
		presentedLayout = [];
		if (closing) {
			closing = false;
			stopStatus();
			dirty = false;
			update({ availability: 'disposed', installed: false, presentedViews: [], error });
			return;
		}
		if (state.session) cleanupSessions.add(state.session);
		stopHeartbeat();
		stopStatus();
		dirty = false;
		update({
			availability: disposed ? 'disposed' : 'unavailable',
			session: null,
			visibleViews: 0,
			installed: false,
			presentedViews: [],
			error
		});
		resolveConnection?.(false);
		resolveConnection = undefined;
		if (!disposed && transport.available() && reconnectTimer === null) {
			reconnectTimer = setTimeout(() => {
				reconnectTimer = null;
				void connect(true);
			}, retryDelay);
			retryDelay = Math.min(8000, retryDelay * 2);
		}
	}
	function measure(): PreviewView[] {
		if (registrations.size > maximumViews) throw new Error('Native preview capacity exceeded');
		return [...registrations.values()].map((registration) => ({
			id: registration.id,
			source: registration.source,
			overlays: [],
			overlay_capacity: 0,
			tile_groups: null,
			...registration.measure()
		}));
	}
	function acknowledge(response: Extract<PreviewResponse, { kind: 'Applied' }>) {
		// Host clipping can resolve fewer views than the browser measured. The aggregate reply
		// cannot identify that subset, so retain presentation only when every visible view agrees.
		if (response.installed)
			presentedLayout =
				response.visible_views === lastViews.filter((view) => view.visible).length ? lastViews : [];
		update({
			acknowledgedRevision: response.revision,
			visibleViews: response.visible_views,
			installed: response.installed,
			presentedViews: retainPresentation(lastViews),
			error: null
		});
		if (closing && response.installed && response.visible_views === 0) {
			closing = false;
			stopStatus();
			cleanupSessions.add(state.session!);
			update({ session: null });
		} else if (!response.installed) scheduleStatus();
	}
	async function pump() {
		if (running) return;
		running = true;
		try {
			while (true) {
				const cleanup = cleanupSessions.values().next().value as string | undefined;
				if (cleanup) {
					cleanupSessions.delete(cleanup);
					try {
						await transport.request({ kind: 'Disconnect', session: cleanup });
					} catch {
						/* Lease expiry remains the native fallback. */
					}
					continue;
				}
				if (disposed && !closing) break;
				const turn = generation;
				let request: PreviewRequest;
				if (connectWanted) {
					connectWanted = false;
					request = { kind: 'Connect', client, version: PREVIEW_PROTOCOL_VERSION };
				} else if (dirty && (state.availability === 'available' || closing) && state.session) {
					dirty = false;
					if (revision === 0xffffffff) {
						fail('Native preview revision exhausted');
						continue;
					}
					try {
						const geometry = JSON.stringify(measure());
						// Freeze the exact cohort before yielding; reactive overlay/tile arrays may
						// change while the browser completes the rendering opportunity.
						const views = JSON.parse(geometry) as PreviewView[];
						if (geometry !== lastGeometry) {
							lastGeometry = geometry;
							update({ installed: false, presentedViews: retainPresentation(views) });
						}
						lastViews = views;
						stopStatus();
						statusAttempts = 0;
						request = { kind: 'Layout', session: state.session, revision: ++revision, views };
					} catch (error) {
						fail(String(error));
						continue;
					}
				} else if (
					statusWanted &&
					(state.availability === 'available' || closing) &&
					state.session
				) {
					statusWanted = false;
					statusAttempts++;
					request = { kind: 'Status', session: state.session, revision };
				} else {
					if (closing && statusTimer === null) fail('Native preview clear could not be confirmed');
					break;
				}
				if (request.kind === 'Layout' && request.views.some((view) => view.visible)) {
					const ready = await rendering.wait();
					if (turn !== generation || (disposed && !closing)) continue;
					if (request.views.some((view) => registrations.get(view.id)?.source !== view.source)) {
						dirty = true;
						continue;
					}
					if (!ready) {
						fail('Native preview rendering opportunity timed out');
						continue;
					}
				}
				let response: PreviewResponse;
				// Withhold capability on timeout, but retain the serialized IPC slot until its
				// actual promise settles. Restarting parallel calls would defeat boundedness.
				const timeout = setTimeout(() => {
					if (turn === generation && (!disposed || closing)) {
						generation++;
						fail('Native preview response timed out');
					}
				}, 1500);
				try {
					response = await transport.request(request);
				} catch (error) {
					if (turn === generation && (!disposed || closing)) fail(String(error));
					continue;
				} finally {
					clearTimeout(timeout);
				}
				if (!response || typeof response !== 'object') {
					if (turn === generation && (!disposed || closing))
						fail('Native preview returned an invalid response');
					continue;
				}
				if ((disposed && !closing) || turn !== generation) {
					if (response.kind === 'Connected') {
						if (closing && !state.session && response.version === PREVIEW_PROTOCOL_VERSION) {
							revision = 0;
							update({ session: response.session });
							dirty = true;
						} else cleanupSessions.add(response.session);
					}
					continue;
				}
				if (
					request.kind === 'Connect' &&
					response.kind === 'Connected' &&
					response.version === PREVIEW_PROTOCOL_VERSION &&
					typeof response.session === 'string' &&
					response.session.length > 0 &&
					Number.isSafeInteger(response.max_views) &&
					response.max_views > 0
				) {
					maximumViews = Math.min(MAX_VIEWS, response.max_views);
					revision = 0;
					cancelFrame();
					retryDelay = 1000;
					stopReconnect();
					lastGeometry = '';
					lastViews = [];
					presentedLayout = [];
					pendingApplied = null;
					update({
						availability: 'available',
						session: response.session,
						acknowledgedRevision: 0,
						visibleViews: 0,
						installed: false,
						presentedViews: [],
						error: null
					});
					resolveConnection?.(true);
					resolveConnection = undefined;
					dirty = true;
					startHeartbeat();
				} else if (
					(request.kind === 'Layout' || request.kind === 'Status') &&
					response.kind === 'Applied' &&
					response.revision === request.revision &&
					Number.isSafeInteger(response.visible_views) &&
					response.visible_views >= 0 &&
					response.visible_views <= MAX_VIEWS &&
					typeof response.installed === 'boolean'
				) {
					const current = request.revision === revision && !dirty;
					if (current && frame === null) acknowledge(response);
					else {
						// A queued DOM measurement may be unrelated. Keep the reply withheld until
						// that frame proves the desired geometry unchanged, rather than losing it.
						pendingApplied = current ? response : null;
						update({ installed: false });
					}
				} else {
					if (
						response.kind === 'Connected' &&
						typeof response.session === 'string' &&
						response.session.length
					)
						cleanupSessions.add(response.session);
					fail(
						response.kind === 'Rejected'
							? `Native preview rejected: ${response.code}`
							: 'Native preview returned an invalid response'
					);
				}
			}
		} finally {
			running = false;
			settle();
		}
	}
	function schedule(hide: boolean) {
		if (disposed || frame !== null) return;
		const callback = () => {
			frame = null;
			if (hide) {
				try {
					const views = measure();
					if (JSON.stringify(views) === lastGeometry) {
						if (pendingApplied && pendingApplied.revision === revision && !dirty) {
							const response = pendingApplied;
							pendingApplied = null;
							acknowledge(response);
						} else if (
							!state.installed &&
							!running &&
							state.availability === 'available' &&
							revision > 0
						)
							scheduleStatus();
						settle();
						return;
					}
					pendingApplied = null;
					update({ installed: false, presentedViews: retainPresentation(views) });
				} catch (error) {
					fail(String(error));
					void pump();
					return;
				}
			}
			dirty = true;
			void pump();
			settle();
		};
		frame =
			typeof requestAnimationFrame === 'function'
				? requestAnimationFrame(callback)
				: setTimeout(callback, 0);
	}
	function invalidate() {
		schedule(true);
	}
	function connect(force: boolean): Promise<boolean> {
		if (disposed) return Promise.resolve(false);
		if (!force && (state.availability === 'available' || state.availability === 'connecting'))
			return connection;
		resolveConnection?.(false);
		generation++;
		rendering.cancel();
		pendingApplied = null;
		if (state.session) cleanupSessions.add(state.session);
		stopHeartbeat();
		stopStatus();
		stopReconnect();
		if (!transport.available()) {
			fail('Native preview is unavailable');
			connection = Promise.resolve(false);
			return connection;
		}
		client = newId();
		presentedLayout = [];
		connectWanted = true;
		update({
			availability: 'connecting',
			session: null,
			acknowledgedRevision: 0,
			visibleViews: 0,
			installed: false,
			presentedViews: [],
			error: null
		});
		connection = new Promise((resolve) => {
			resolveConnection = resolve;
		});
		void pump();
		return connection;
	}
	function whenSettled(): Promise<void> {
		return new Promise((resolve) => {
			idleWaiters.add(resolve);
			settle();
		});
	}

	return {
		get state() {
			return state;
		},
		connect: () => connect(false),
		reconnect: () => connect(true),
		invalidate,
		whenSettled,
		subscribe(listener) {
			listeners.add(listener);
			listener(state);
			return () => {
				listeners.delete(listener);
			};
		},
		register(source, measureGeometry) {
			if (disposed) throw new Error('Native preview session is disposed');
			if (registrations.size >= maximumViews) throw new Error('Native preview capacity exceeded');
			const registration = { id: newId(), source, measure: measureGeometry };
			registrations.set(registration.id, registration);
			invalidate();
			let active = true;
			return {
				id: registration.id,
				async pointer(x, y) {
					const session = state.session,
						epoch = generation,
						source = registration.source;
					if (!active || disposed || !session || state.availability !== 'available') return null;
					try {
						const reply = await transport.request({
							kind: 'Pointer',
							session,
							view: registration.id,
							source,
							point: [x, y]
						});
						if (
							!active ||
							disposed ||
							epoch !== generation ||
							session !== state.session ||
							source !== registration.source
						)
							return null;
						return reply.kind === 'Pointer' ? reply.sample : null;
					} catch {
						return null;
					}
				},
				update(source) {
					if (!disposed && active && registration.source !== source) {
						registration.source = source;
						rendering.cancel();
						update({ installed: false });
						invalidate();
					}
				},
				invalidate,
				unregister() {
					if (active) {
						active = false;
						registrations.delete(registration.id);
						if (disposed) return;
						rendering.cancel();
						update({
							installed: false,
							presentedViews: state.presentedViews.filter((id) => id !== registration.id)
						});
						invalidate();
					}
				}
			};
		},
		dispose() {
			if (!disposed) {
				disposed = true;
				closing = Boolean(state.session) || running;
				generation++;
				rendering.cancel();
				pendingApplied = null;
				connectWanted = false;
				dirty = closing;
				cancelFrame();
				stopHeartbeat();
				stopStatus();
				stopReconnect();
				registrations.clear();
				resolveConnection?.(false);
				resolveConnection = undefined;
				presentedLayout = [];
				update({ availability: 'disposed', visibleViews: 0, installed: false, presentedViews: [] });
				void pump();
			}
			return whenSettled();
		}
	};
}
