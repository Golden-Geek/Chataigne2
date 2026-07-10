import { PanelRegistry } from "@golden/ui";

export const CHATAIGNE_UI_BOUNDARY = "chataigne-ui" as const;

export function registerChataignePanels(registry: PanelRegistry): void {
  for (const panel of [
    { id: "chataigne.modules", kind: "workspace", title: "Modules" },
    { id: "chataigne.module-inspector", kind: "inspector", title: "Module Inspector" },
    { id: "chataigne.state-machine", kind: "workspace", title: "State Machine" },
    { id: "chataigne.processor-inspector", kind: "inspector", title: "Processor" },
    { id: "chataigne.dashboard", kind: "dashboard", title: "Dashboard" },
    { id: "chataigne.spatializer", kind: "workspace", title: "Spatializer" },
  ] as const) {
    registry.register(panel);
  }
}
