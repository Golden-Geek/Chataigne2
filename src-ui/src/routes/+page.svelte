<script lang="ts">
	import TerminalPanel from "$lib/panels/TerminalPanel.svelte";
	import { MainWindow, type UserPanelDefinitionMap } from "$lib/golden_ui";
	import { appIcons } from "$lib/assets/icons/node-icons.svelte";

	const userPanels: UserPanelDefinitionMap = {
		terminal: {
			title: "Terminal",
			description: "User panel example",
			component: TerminalPanel,
			defaultParams: {
				lines: [
					"[info] Session created",
					"[info] Panels connected",
					"[warn] No backend signal",
				],
			},
		},
	};


</script>

<MainWindow {userPanels} nodeIcons={appIcons} />
