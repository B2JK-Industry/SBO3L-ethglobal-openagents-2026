import { describe, it, expect } from "@jest/globals";
import { priceIdFor, tierFromPriceId, quotaPctOfHard, TIER_QUOTA } from "../lib/stripe";

describe("priceIdFor / tierFromPriceId round trip", () => {
  it("maps free → price → free", () => {
    expect(tierFromPriceId(priceIdFor("free"))).toBe("free");
  });
  it("maps pro → price → pro", () => {
    expect(tierFromPriceId(priceIdFor("pro"))).toBe("pro");
  });
  it("maps enterprise → price → enterprise", () => {
    expect(tierFromPriceId(priceIdFor("enterprise"))).toBe("enterprise");
  });
  it("returns null for unknown price IDs", () => {
    expect(tierFromPriceId("price_unknown_garbage")).toBeNull();
  });
});

describe("quotaPctOfHard", () => {
  it("returns 0 for enterprise (unlimited)", () => {
    expect(quotaPctOfHard("enterprise", 1_000_000)).toBe(0);
  });
  it("clamps to 100 when over hard cap", () => {
    expect(quotaPctOfHard("free", TIER_QUOTA.free.hard_per_day * 2)).toBe(100);
  });
  it("midpoint pro returns 50", () => {
    expect(quotaPctOfHard("pro", TIER_QUOTA.pro.hard_per_day / 2)).toBe(50);
  });
});
