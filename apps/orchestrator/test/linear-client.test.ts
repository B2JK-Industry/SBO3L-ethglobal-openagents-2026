import { describe, expect, it } from "vitest";

import { pickHighestPriority } from "../src/linear-client.js";
import type { LinearIssue } from "../src/types.js";

function issue(id: string, priority: number): LinearIssue {
  return {
    id,
    identifier: id,
    title: `t-${id}`,
    priority,
    state: { id: "s", name: "Todo", type: "unstarted" },
  };
}

describe("pickHighestPriority", () => {
  it("returns null on empty input", () => {
    expect(pickHighestPriority([])).toBeNull();
  });

  it("prefers urgent (1) over high/medium/low (2/3/4)", () => {
    const picked = pickHighestPriority([issue("a", 3), issue("b", 1), issue("c", 4)]);
    expect(picked?.id).toBe("b");
  });

  it("treats priority 0 (none) as the lowest", () => {
    const picked = pickHighestPriority([issue("a", 0), issue("b", 4)]);
    expect(picked?.id).toBe("b");
  });

  it("preserves stable order on ties", () => {
    const picked = pickHighestPriority([issue("a", 2), issue("b", 2)]);
    expect(picked?.id).toBe("a");
  });
});
