import { ApiError } from "../src/lib/api";

describe("ApiError", () => {
  it("formats status + body in message", () => {
    const e = new ApiError(403, "forbidden");
    expect(e.message).toBe("SBO3L API error 403: forbidden");
    expect(e.status).toBe(403);
    expect(e.body).toBe("forbidden");
  });

  it("is instanceof Error", () => {
    expect(new ApiError(500, "boom") instanceof Error).toBe(true);
  });
});
