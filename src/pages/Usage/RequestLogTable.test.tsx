import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, waitFor } from "@testing-library/react";
import { RequestLogTable } from "./RequestLogTable";

// mock ipc:拦截 getRequestLogs / exportRequestLogsCsv,其它保留
vi.mock("@/lib/ipc", async () => {
  const actual = await vi.importActual<typeof import("@/lib/ipc")>("@/lib/ipc");
  return {
    ...actual,
    getRequestLogs: vi.fn(),
    exportRequestLogsCsv: vi.fn(),
  };
});

// mock i18n:避免 useTranslation 在 jsdom 跑 real i18next 初始化
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "zh" },
  }),
}));

import * as ipc from "@/lib/ipc";

const mockedGetRequestLogs = vi.mocked(ipc.getRequestLogs);
const mockedExportRequestLogsCsv = vi.mocked(ipc.exportRequestLogsCsv);

const EMPTY_PAGINATED: ipc.PaginatedLogs = { rows: [], total: 0, page: 1, pageSize: 20 };

describe("RequestLogTable filter regression (#17)", () => {
  beforeEach(() => {
    mockedGetRequestLogs.mockReset();
    mockedExportRequestLogsCsv.mockReset();
    mockedGetRequestLogs.mockResolvedValue(EMPTY_PAGINATED);
    mockedExportRequestLogsCsv.mockResolvedValue("");
  });

  it("appFilter='', modelFilter='' → getRequestLogs 收到 undefined(appType/model 都是 undefined,不是 '')", async () => {
    render(<RequestLogTable startDate={undefined} appFilter="" modelFilter="" />);
    await waitFor(() => {
      expect(mockedGetRequestLogs).toHaveBeenCalled();
    });
    const filters = mockedGetRequestLogs.mock.calls[0][0] as ipc.RequestLogFilters;
    expect(filters.appType).toBeUndefined();
    expect(filters.model).toBeUndefined();
    // 显式断言 NOT 空串 —— 后端 SQL `app_type = ''` 永远 false 是 #17 的 bug 触发点
    expect(filters.appType).not.toBe("");
    expect(filters.model).not.toBe("");
  });

  it("appFilter='claude', modelFilter='sonnet-4' → 正常透传", async () => {
    render(
      <RequestLogTable
        startDate={undefined}
        appFilter="claude"
        modelFilter="sonnet-4"
      />,
    );
    await waitFor(() => {
      expect(mockedGetRequestLogs).toHaveBeenCalled();
    });
    const filters = mockedGetRequestLogs.mock.calls[0][0] as ipc.RequestLogFilters;
    expect(filters.appType).toBe("claude");
    expect(filters.model).toBe("sonnet-4");
  });

  it("appFilter='opencode', modelFilter='' → appType 透传,model=undefined", async () => {
    render(
      <RequestLogTable
        startDate={undefined}
        appFilter="opencode"
        modelFilter=""
      />,
    );
    await waitFor(() => {
      expect(mockedGetRequestLogs).toHaveBeenCalled();
    });
    const filters = mockedGetRequestLogs.mock.calls[0][0] as ipc.RequestLogFilters;
    expect(filters.appType).toBe("opencode");
    expect(filters.model).toBeUndefined();
    expect(filters.model).not.toBe("");
  });
});