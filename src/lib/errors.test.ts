import { describe, expect, it } from "vitest";
import { describeError } from "./errors";

describe("describeError", () => {
  it("returns 未知错误 for null and undefined", () => {
    expect(describeError(null)).toBe("未知错误");
    expect(describeError(undefined)).toBe("未知错误");
  });

  it("passes through strings as-is", () => {
    expect(describeError("npx not found")).toBe("npx not found");
    expect(describeError("")).toBe("");
  });

  it("extracts message from native Error", () => {
    expect(describeError(new Error("boom"))).toBe("boom");
    expect(describeError(new TypeError("bad type"))).toBe("bad type");
  });

  it("handles Error with empty message by returning 未知错误", () => {
    expect(describeError(new Error(""))).toBe("未知错误");
  });

  it("extracts message from Tauri IPCError-like shape (object with .message)", () => {
    // Tauri 把后端 Result<_, String> 包成 IPCError,实际到前端时 e 是 Error 但 message 是后端串
    const e = new Error("server start failed: not found");
    expect(describeError(e)).toBe("server start failed: not found");
  });

  it("extracts .message from plain object", () => {
    expect(describeError({ message: "rate limit" })).toBe("rate limit");
  });

  it("extracts .error from plain object (axios-ish)", () => {
    expect(describeError({ error: "401 unauthorized" })).toBe("401 unauthorized");
  });

  it("falls back to toString() when message/error missing", () => {
    class CustomErr {
      toString() {
        return "custom err";
      }
    }
    expect(describeError(new CustomErr())).toBe("custom err");
  });

  it("ignores [object Object] toString and JSON-stringifies", () => {
    expect(describeError({ code: 42, kind: "weird" })).toBe(
      JSON.stringify({ code: 42, kind: "weird" }),
    );
  });

  it("handles numbers and booleans via String()", () => {
    expect(describeError(42)).toBe("42");
    expect(describeError(false)).toBe("false");
  });

  it("handles throwing toString gracefully", () => {
    const evil = {
      message: "ok msg",
      toString() {
        throw new Error("toString blew up");
      },
    };
    // .message wins because it's checked first
    expect(describeError(evil)).toBe("ok msg");
  });

  it("falls back to 未知错误 when toString throws and no message/error", () => {
    const evil = {
      toString() {
        throw new Error("nope");
      },
    };
    expect(describeError(evil)).toBe("未知错误");
  });

  it("handles circular references in JSON.stringify fallback", () => {
    const a: any = { foo: 1 };
    a.self = a; // circular
    // 没有 message/error 也没有能用的 toString,所以走 JSON.stringify → 抛 → 兜底
    expect(describeError(a)).toBe("未知错误");
  });
});