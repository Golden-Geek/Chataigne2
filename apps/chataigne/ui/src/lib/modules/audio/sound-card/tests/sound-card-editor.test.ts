import { render } from 'svelte/server';
import type { PanelParams, PanelProps } from 'golden_ui';
import { describe, expect, it, vi } from 'vitest';
import SoundCardEditorPanel from '$lib/panels/modules/SoundCardEditorPanel.svelte';

const props: PanelProps = {
	panelId: 'sound-card-editor-test',
	panelType: 'soundCardEditor',
	title: 'Sound Card',
	params: {},
	panelApi: {
		setTitle: vi.fn(),
		close: vi.fn(),
		updateParams: vi.fn(),
		getParams: <T extends PanelParams = PanelParams>() => ({}) as T
	}
};

describe('Sound Card editor shell', () => {
	it('stays focused on the simplified backend-owned module tree', () => {
		const body = render(SoundCardEditorPanel, { props }).body;

		expect(body).toContain('Connection, routing, channel levels, and processing.');
		expect(body).not.toMatch(/virtual|profile|monitoring|playback|diagnostic/i);
	});
});
