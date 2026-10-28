import type { UiApplicationInfoDto } from '../generated/rust_protocol/UiApplicationInfoDto';
import { validateApplicationInfo } from '../host/application';
import { resetProjectFileFormat } from './project-file-format.svelte';

export const applicationState = $state<{ info: UiApplicationInfoDto | null }>({
	info: null
});

export const configureApplication = (application: UiApplicationInfoDto): void => {
	const next = validateApplicationInfo(application);
	if (applicationState.info?.id !== next.id) resetProjectFileFormat();
	applicationState.info = next;
};

export const applicationStorageKey = (suffix: string): string | null => {
	const identity = applicationState.info?.id;
	return identity === undefined ? null : `golden_ui.${encodeURIComponent(identity)}.${suffix}`;
};
