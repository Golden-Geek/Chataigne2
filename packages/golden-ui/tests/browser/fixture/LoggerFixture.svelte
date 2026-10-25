<script lang="ts">
	import { onDestroy, tick } from 'svelte';
	import LoggerPanel from '../../../components/panels/logger/LoggerPanel.svelte';
	import { appState } from '../../../store/app-state.svelte';
	import type { UiLogRecord } from '../../../types';
	import type { PanelApi } from '../../../dockview/panel-types';
	import { createFixtureSession } from './session';
	import { measureCollectionFlow } from './timing';
	let { panelApi }: { panelApi: PanelApi } = $props();
	let fixture = createFixtureSession();
	let panel = $state<ReturnType<typeof LoggerPanel>>();
	let size = 0;
	let serial = 0;
	let panelEpoch = $state(0);
	const previousSession = appState.session;
	onDestroy(() => {
		if (appState.session === fixture.session) appState.session = previousSession;
	});
	const frames = async () => {
		await tick();
		await new Promise(requestAnimationFrame);
		await tick();
		await new Promise(requestAnimationFrame);
	};
	const text = (id: number) => `Log ${String(id).padStart(6, '0')}`;
	export async function setup(count: number, unlimited = false, duplicates = false) {
		const started = performance.now();
		size = count;
		fixture = createFixtureSession();
		fixture.load(1);
		appState.session = fixture.session;
		panelEpoch += 1;
		const records: UiLogRecord[] = Array.from({ length: count }, (_, index) => ({
			id: index + 1,
			timestamp_ms: 1_700_000_000_000 + index,
			level: 'info',
			tag: duplicates ? 'fixture' : index % 3 ? 'stream' : 'special',
			message: duplicates
				? `Repeated ${index % 10}`
				: text(index + 1) +
					(index % 13 === 0 ? '\n' + 'A wrapped multiline log message. '.repeat(18) : ''),
			origin: !duplicates && index % 2 ? 2 : undefined,
			repeat_count: duplicates ? 2 : 1
		}));
		await fixture.loadLogger(records, unlimited ? 0 : count);
		await tick();
		panel!.searchLogs({});
		panel!.resyncToLatest();
		await frames();
		return { records: count, readyMs: Math.round((performance.now() - started) * 1000) / 1000 };
	}
	export async function browse(id: number) {
		const found = panel!.browseToRecord(`r:${id}`);
		await frames();
		return found;
	}
	export async function focus(id: number) {
		panel!.focusRecord(`r:${id}`);
		await frames();
	}
	export async function search(content: string, source = '', tag = '') {
		panel!.searchLogs({ content, source, tag });
		await frames();
	}
	export async function append() {
		const message = `Appended ${++serial}`;
		fixture.session.appendUiLogRecord('info', 'stream', message);
		await frames();
		return { message, id: fixture.session.logRecords.at(-1)!.id };
	}
	export async function live() {
		panel!.resyncToLatest();
		await frames();
	}
	export async function clear() {
		await fixture.loadLogger([], size);
		await frames();
	}
	export const copyText = () => panel!.displayedLogText();
	export async function collapse() {
		panel!.toggleCollapse();
		await frames();
	}
	export async function superCollapse() {
		panel!.toggleSuperCollapse();
		await frames();
	}
	export async function measureFilter() {
		await search('');
		await live();
		return measureCollectionFlow(
			'log-filter',
			size,
			(index) => panel!.searchLogs({ content: index % 2 ? '' : text(size) }),
			(index) => {
				const list = document.querySelector<HTMLElement>('.logger-list');
				const total = Number(list?.querySelector<HTMLElement>('.virtual-rows')?.dataset.rowCount);
				const target = list?.querySelector<HTMLElement>(`[data-record-key="r:${size}"] .message`);
				if (
					!list ||
					!target ||
					(index % 2 ? total !== size : total !== 1) ||
					!target.textContent?.includes(text(size))
				)
					return false;
				const bounds = list.getBoundingClientRect(),
					rect = target.getBoundingClientRect();
				return rect.width > 0 && rect.bottom > bounds.top && rect.top < bounds.bottom;
			}
		);
	}
	export async function measureAppend() {
		await search('');
		await live();
		let expected = '';
		return measureCollectionFlow(
			'log-append-batch',
			size,
			() => {
				for (let record = 0; record < 4; record += 1) {
					expected = `Batch ${++serial}`;
					fixture.session.appendUiLogRecord('info', 'stream', expected);
				}
			},
			() => {
				const id = fixture.session.logRecords.at(-1)!.id;
				const list = document.querySelector<HTMLElement>('.logger-list');
				const target = list?.querySelector<HTMLElement>(`[data-record-key="r:${id}"] .message`);
				if (!list || target?.textContent !== expected || fixture.session.logRecords.length !== size)
					return false;
				const bounds = list.getBoundingClientRect(),
					rect = target.getBoundingClientRect();
				return (
					rect.width > 0 &&
					rect.bottom > bounds.top &&
					rect.top < bounds.bottom &&
					list.scrollHeight - list.clientHeight - list.scrollTop <= 6
				);
			}
		);
	}
</script>

<div class="logger-scene">
	{#key panelEpoch}
		<LoggerPanel
			bind:this={panel}
			{panelApi}
			params={{}}
			panelId="logger-fixture"
			panelType="logger"
			title="Logs" />
	{/key}
</div>

<style>
	.logger-scene {
		inline-size: 60rem;
		block-size: 32rem;
	}
</style>
