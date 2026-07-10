<script lang="ts">
	import ConfirmationDialog from '../common/ConfirmationDialog.svelte';
	import {
		cancelWindowExit,
		confirmWindowExitDiscard,
		confirmWindowExitSave,
		getWindowExitProjectName,
		windowExitState
	} from '../../store/window-exit.svelte';

	const projectName = $derived(getWindowExitProjectName());
</script>

<ConfirmationDialog
	open={windowExitState.open}
	title="Save changes before exiting?"
	message={`\"${projectName}\" has unsaved changes. Save before closing the app, discard the changes, or stay here.`}
	errorMessage={windowExitState.errorMessage}
	actions={[
		{
			id: 'save',
			label: 'Save',
			shortcut: 'S / Enter',
			tone: 'primary',
			defaultFocus: true,
			disabled: windowExitState.busy,
			action: () => void confirmWindowExitSave()
		},
		{
			id: 'discard',
			label: 'Discard',
			shortcut: 'D / Backspace',
			tone: 'danger',
			disabled: windowExitState.busy,
			action: () => void confirmWindowExitDiscard()
		},
		{
			id: 'cancel',
			label: 'Cancel',
			shortcut: 'C / Escape',
			disabled: windowExitState.busy,
			action: cancelWindowExit
		}
	]} />
