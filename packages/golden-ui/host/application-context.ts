import { getContext, setContext } from 'svelte';
import type { UiApplicationInfoDto } from '../generated/rust_protocol/UiApplicationInfoDto';
import type { RuntimeEndpoints } from './runtime-endpoints';

export interface ApplicationHost {
	readonly application: UiApplicationInfoDto;
	readonly endpoints: RuntimeEndpoints;
}

const APPLICATION_HOST = Symbol('golden-ui.application-host');

export const provideApplicationHost = (host: ApplicationHost): void => {
	setContext(APPLICATION_HOST, host);
};

export const useApplicationHost = (): ApplicationHost => {
	const host = getContext<ApplicationHost | undefined>(APPLICATION_HOST);
	if (!host) throw new Error('ApplicationRoot must own the workbench routes');
	return host;
};
