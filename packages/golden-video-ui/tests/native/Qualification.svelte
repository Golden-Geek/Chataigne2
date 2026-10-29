<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
	import { NativePreview, createDesktopPreviewTransport, createPreviewSession } from '../../index';
	import type { PreviewRequest } from '../../generated/PreviewRequest';
	import type { QualificationCommand } from './generated/QualificationCommand';
	import type { QualificationLayout } from './generated/QualificationLayout';
	import type { QualificationPhase } from './generated/QualificationPhase';
	import type { QualificationReport } from './generated/QualificationReport';
	import {
		QUALIFICATION_COMMAND_EVENT,
		QUALIFICATION_LAYOUT_EVENT,
		QUALIFICATION_REPORT_EVENT
	} from './generated/constants';
	import {
		emptyQualificationReport,
		qualificationCommand,
		qualificationLayout,
		queryQualificationLayout
	} from './qualification';

	const desktop = createDesktopPreviewTransport();
	let lastLayout: Extract<PreviewRequest, { kind: 'Layout' }> | null = null;
	const session = createPreviewSession({
		available: desktop.available,
		async request(request) {
			if (request.kind === 'Layout') lastLayout = request;
			return desktop.request(request);
		}
	});
	let layout = $state<QualificationLayout | null>(null);
	let hidden = $state(false);
	let phase = $state<QualificationPhase>('listening');
	let overlayClicks = $state<[number, number]>([0, 0]);
	const report = emptyQualificationReport();
	(
		window as typeof window & { __previewQualification: QualificationReport }
	).__previewQualification = report;
	let connected = false;
	let cancelled = false;
	let disconnected = false;
	let running = false;
	let wantedRevision = -1;
	let wantedCommand = -1;
	let pendingLayout: QualificationLayout | null = null;
	let pendingCommand: QualificationCommand | null = null;

	async function record(next: QualificationPhase, commandId: number | null = null) {
		const masks = document.querySelectorAll('[data-native-preview-retirement]').length;
		Object.assign(report, {
			sequence: report.sequence + 1,
			commandId,
			phase: next,
			layout: layout ? structuredClone($state.snapshot(layout)) : null,
			installed: session.state.installed,
			visibleViews: session.state.visibleViews,
			views: structuredClone(lastLayout?.views ?? []),
			retirementMasks: masks,
			overlayClicks: [...overlayClicks]
		});
		phase = next;
		if (desktop.available()) await emit(QUALIFICATION_REPORT_EVENT, report);
	}

	async function failed(error: unknown) {
		report.error = String(error).slice(0, 8192);
		await record('failed');
	}

	async function settled() {
		await tick();
		session.invalidate();
		await session.whenSettled();
		if (!session.state.installed || session.state.error)
			throw new Error(session.state.error ?? 'Native fixture installation was not confirmed');
		if (document.querySelector('[data-native-preview-retirement]'))
			throw new Error('Retirement masks remained after native installation');
	}

	async function pump() {
		if (running || !connected || cancelled || report.error) return;
		running = true;
		try {
			while (!cancelled && (pendingLayout || pendingCommand)) {
				if (pendingLayout) {
					if (disconnected) throw new Error('Layout arrived after fixture disconnection');
					const desired = pendingLayout;
					pendingLayout = null;
					layout = desired;
					await settled();
					if (desired.revision === wantedRevision) await record(hidden ? 'hidden' : 'ready');
					continue;
				}
				const command = pendingCommand!;
				pendingCommand = null;
				if (!layout) throw new Error('Fixture command requires a host layout');
				if (disconnected) throw new Error('Fixture session is already disconnected');
				if (command.action === 'disconnect') {
					await session.dispose();
					if (!session.state.installed || session.state.error)
						throw new Error(session.state.error ?? 'Fixture clear was not acknowledged');
					disconnected = true;
					await record('disconnected', command.id);
				} else {
					if (command.action === 'ready' && !(await session.reconnect()))
						throw new Error('Native fixture session could not be renegotiated');
					if (command.action === 'hide') hidden = true;
					if (command.action === 'show') hidden = false;
					await settled();
					await record(
						command.action === 'hide' ? 'hidden' : command.action === 'show' ? 'shown' : 'ready',
						command.id
					);
				}
			}
		} catch (error) {
			await failed(error);
		} finally {
			running = false;
		}
	}

	function receiveLayout(value: unknown) {
		try {
			const desired = qualificationLayout(value);
			if (desired.revision <= wantedRevision) return;
			wantedRevision = desired.revision;
			pendingLayout = desired;
			void pump();
		} catch (error) {
			void failed(error);
		}
	}
	function receiveCommand(value: unknown) {
		try {
			const command = qualificationCommand(value);
			if (command.id <= wantedCommand) return;
			if (pendingCommand) throw new Error('Fixture command capacity exceeded');
			wantedCommand = command.id;
			pendingCommand = command;
			void pump();
		} catch (error) {
			void failed(error);
		}
	}

	onMount(() => {
		const listeners: UnlistenFn[] = [];
		const masks = new MutationObserver(() => {
			report.maxRetirementMasks = Math.max(
				report.maxRetirementMasks,
				document.querySelectorAll('[data-native-preview-retirement]').length
			);
			if (report.maxRetirementMasks > 64) void failed('Retirement mask capacity exceeded');
		});
		masks.observe(document.body, { childList: true, subtree: true });
		void (async () => {
			try {
				if (!desktop.available()) throw new Error('Native preview capability unavailable');
				for (const [event, receive] of [
					[QUALIFICATION_LAYOUT_EVENT, receiveLayout],
					[QUALIFICATION_COMMAND_EVENT, receiveCommand]
				] as const) {
					const unlisten = await listen(event, (message) => receive(message.payload));
					if (cancelled) unlisten();
					else listeners.push(unlisten);
				}
				if (cancelled) return;
				await record('listening');
				if (!(await session.connect())) throw new Error('Native preview capability unavailable');
				connected = true;
				const query = queryQualificationLayout();
				if (query) receiveLayout(query);
				await pump();
			} catch (error) {
				await failed(error);
			}
		})();
		return () => {
			cancelled = true;
			masks.disconnect();
			for (const unlisten of listeners) unlisten();
			void session.dispose();
		};
	});
</script>

<section class="chrome">
	<h1>Held native preview fixture</h1>
	<p data-qualification-phase={phase}>{phase}: CSS geometry awaits host physical validation.</p>
</section>
<div class="views" class:hidden>
	{#if layout}
		{#each layout.views as view, index (index)}
			<!-- Host-computed CSS dimensions deliberately implement the fixture's measured-size contract. -->
			<div
				class="preview"
				data-qualification-source={view.source}
				style:width={`${view.cssWidth}px`}
				style:height={`${view.cssHeight}px`}
			>
				<NativePreview
					{session}
					source={view.source}
					visible={!hidden}
					label={`Native ${view.source}`}
				>
					<svg viewBox="0 0 160 90" aria-hidden="true">
						<path d="M2 2H158V88H2ZM80 2V88M2 45H158" />
					</svg>
				</NativePreview>
				<button
					type="button"
					onclick={() => overlayClicks[index]++}
					aria-label={`Overlay ${index + 1}`}>{index + 1}: {overlayClicks[index]}</button
				>
			</div>
		{/each}
	{/if}
</div>

<style>
	:global(html),
	:global(body),
	:global(#app) {
		margin: 0;
		min-height: 100%;
		background: transparent;
		color: #f2f4f7;
		font-family: sans-serif;
	}
	.chrome {
		margin: 1rem;
		padding: 0.5rem;
		background: #242934;
	}
	h1 {
		margin: 0;
		font-size: 1.1rem;
	}
	p {
		margin-block: 0.5rem 0;
	}
	.views {
		display: flex;
		align-items: start;
		gap: 1rem;
		margin: 1rem;
		width: max-content;
	}
	.hidden {
		visibility: hidden;
	}
	.preview {
		position: relative;
		flex: none;
	}
	svg {
		width: 100%;
		height: 100%;
		fill: none;
		stroke: white;
		stroke-width: 0.5;
	}
	button {
		position: absolute;
		inset: 1rem auto auto 1rem;
		border: 0.1rem solid white;
		border-radius: 0.2rem;
		padding: 0.5rem;
		background: #263146;
		color: white;
		font: inherit;
	}
</style>
