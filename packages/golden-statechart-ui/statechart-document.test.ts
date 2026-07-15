import { describe, expect, it } from "vitest";

import { StatechartDocumentView } from "./statechart-document";

describe("StatechartDocumentView", () => {
  it("keeps identity for an unchanged statechart snapshot and revisions changed snapshots", () => {
    const view = new StatechartDocumentView();
    const states = [];
    const transitions = [];
    const first = view.update({ states, transitions });
    const unchanged = view.update({ states, transitions });
    const changed = view.update({ states: [], transitions });

    expect(unchanged).toBe(first);
    expect(changed.revision.sequence).toBe(first.revision.sequence + 1);
  });
});
