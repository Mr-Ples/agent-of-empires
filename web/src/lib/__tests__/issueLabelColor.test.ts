import { describe, expect, it } from "vitest";

import { issueLabelStyle } from "../issueLabelColor";

describe("issueLabelStyle", () => {
  it("uses the GitHub color with readable white text", () => {
    expect(issueLabelStyle("0e8a16")).toEqual({
      backgroundColor: "#0e8a16",
      borderColor: "#0e8a16",
      color: "#ffffff",
    });
  });

  it("uses dark text for a light label color", () => {
    expect(issueLabelStyle("fbca04")).toEqual({
      backgroundColor: "#fbca04",
      borderColor: "#fbca04",
      color: "#111827",
    });
  });

  it("falls back for missing, malformed, and low-contrast colors", () => {
    expect(issueLabelStyle(null)).toBeUndefined();
    expect(issueLabelStyle("not-a-color")).toBeUndefined();
    expect(issueLabelStyle("777777")).toBeUndefined();
  });
});
