/** App composition sets these before mounting the workbench. Defaults stay caller-owned. */
export type WorkbenchActionIcon = 'remove';
const icons = new Map<WorkbenchActionIcon, string>();

export function registerActionIcon(action: WorkbenchActionIcon, icon: string): void {
	icons.set(action, icon);
}

export function resolveActionIcon(action: WorkbenchActionIcon, fallback: string): string {
	return icons.get(action) ?? fallback;
}
