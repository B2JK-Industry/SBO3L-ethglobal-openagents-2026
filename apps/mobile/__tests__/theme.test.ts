import { tokens } from "../src/theme";

describe("tokens", () => {
  it("matches design-tokens accent + bg", () => {
    expect(tokens.accent).toBe("#4ade80");
    expect(tokens.bg).toBe("#0a0e1a");
  });
  it("exposes radius scale", () => {
    expect(tokens.rSm).toBeLessThan(tokens.rMd);
    expect(tokens.rMd).toBeLessThan(tokens.rLg);
  });
});
