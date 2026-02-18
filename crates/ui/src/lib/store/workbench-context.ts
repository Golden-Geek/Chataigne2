import { getContext, setContext } from 'svelte';
import type { WorkbenchSession } from './workbench.svelte';

const WORKBENCH_CONTEXT_KEY = Symbol('golden-core-ui-workbench');

type WorkbenchSessionGetter = () => WorkbenchSession | null | undefined;

export const setWorkbenchContext = (getSession: WorkbenchSessionGetter): void => {
	setContext(WORKBENCH_CONTEXT_KEY, getSession);
};

export const getWorkbenchContext = (): WorkbenchSession => {
	const getSession = getContext<WorkbenchSessionGetter | undefined>(WORKBENCH_CONTEXT_KEY);
	const session = getSession?.();
	if (!session) {
		throw new Error('Workbench context is missing. Wrap panel components in <Workbench>.');
	}
	return session;
};
