/**
 * stdio ↔ HTTP MCP 转发器
 *
 * 读 stdin 的 newline-delimited JSON-RPC，每行一个请求，独立 POST 到 MS Manage HTTP endpoint。
 * 响应按 SSE / 普通 JSON 解析后写到 stdout（每个 JSON-RPC 响应一行）。
 */

// 显式用 undici 的 fetch，而不是依赖全局 fetch ——
// 全局 fetch 在 Node < 18.14 缺失/实验性，会让旧 Node 上一跑就崩（表现为「连不上」）。
// undici 是本包依赖（要求 Node 18+），其 fetch 与全局 fetch 接口一致（body 为 web ReadableStream）。
import { fetch, type Response } from "undici";

const DEFAULT_TIMEOUT_MS = 30_000;
const DEBUG = process.env["MS_MANAGE_DEBUG"] === "1";

function debug(msg: string, ...rest: unknown[]): void {
  if (!DEBUG) return;
  process.stderr.write(`[ms-manage] ${msg} ${rest.map((x) => JSON.stringify(x)).join(" ")}\n`);
}

function fatal(msg: string): never {
  process.stderr.write(`[ms-manage] fatal: ${msg}\n`);
  process.exit(2);
}

/** 兼容性 AbortSignal.timeout：Node >= 17.3 有原生实现，旧版用手动 controller。 */
function timeoutSignal(ms: number): AbortSignal {
  const nativeTimeout = (AbortSignal as unknown as { timeout?: (m: number) => AbortSignal }).timeout;
  if (typeof nativeTimeout === "function") return nativeTimeout.call(AbortSignal, ms);
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), ms);
  // 定时器不阻止进程退出（请求结束进程自然退出，不必等满超时）。
  if (typeof (t as { unref?: () => void }).unref === "function") (t as { unref: () => void }).unref();
  return ctrl.signal;
}

interface JsonRpcRequest {
  jsonrpc: "2.0";
  id?: string | number | null;
  method: string;
  params?: unknown;
}

interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: string | number | null;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}

function isRequest(x: unknown): x is JsonRpcRequest {
  return typeof x === "object" && x !== null && (x as { jsonrpc?: unknown }).jsonrpc === "2.0"
    && typeof (x as { method?: unknown }).method === "string";
}

function makeError(id: unknown, message: string): string {
  const resp: JsonRpcResponse = {
    jsonrpc: "2.0",
    id: (typeof id === "string" || typeof id === "number") ? id : null,
    error: { code: -32000, message },
  };
  return JSON.stringify(resp) + "\n";
}

async function handleOne(rawLine: string): Promise<void> {
  let req: JsonRpcRequest;
  try {
    const parsed: unknown = JSON.parse(rawLine);
    if (!isRequest(parsed)) throw new Error("not a valid JSON-RPC request");
    req = parsed;
  } catch (e) {
    debug("skip invalid line", rawLine.slice(0, 200));
    return;
  }

  const url = process.env["MS_MANAGE_URL"];
  if (!url) {
    process.stdout.write(makeError(req.id, "MS_MANAGE_URL env not set"));
    return;
  }
  const token = process.env["MS_MANAGE_TOKEN"];
  const timeoutStr = process.env["MS_MANAGE_TIMEOUT"];
  const timeoutMs = timeoutStr ? parseInt(timeoutStr, 10) || DEFAULT_TIMEOUT_MS : DEFAULT_TIMEOUT_MS;

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    "Accept": "application/json, text/event-stream",
  };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  let res: Response;
  try {
    res = await fetch(url, {
      method: "POST",
      headers,
      body: rawLine,
      signal: timeoutSignal(timeoutMs),
    });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    debug("fetch error", msg);
    // 通知（无 id）不期待响应，错误只写 stderr
    if (req.id === undefined || req.id === null) {
      process.stderr.write(`[ms-manage] notification fetch failed (${req.method}): ${msg}\n`);
    } else {
      process.stdout.write(makeError(req.id, `fetch failed: ${msg}`));
    }
    return;
  }

  if (!res.ok) {
    const body = await res.text().catch(() => "");
    const errMsg = `HTTP ${res.status} ${res.statusText}: ${body.slice(0, 500)}`;
    if (req.id === undefined || req.id === null) {
      // 通知的 HTTP 错误不算 RPC 错误，仅写 stderr 提示
      process.stderr.write(`[ms-manage] notification ${req.method} got ${errMsg}\n`);
    } else {
      process.stdout.write(makeError(req.id, errMsg));
    }
    return;
  }

  const ct = res.headers.get("content-type") ?? "";
  if (ct.includes("text/event-stream") && res.body) {
    await consumeSse(res.body, req.id);
  } else {
    const text = await res.text();
    if (text.length > 0) {
      // 末尾可能多个 JSON 拼在一起，按行切（虽然 HTTP 正常只有一个）
      for (const line of text.split("\n")) {
        const t = line.trim();
        if (t) process.stdout.write(t + "\n");
      }
    }
  }
}

async function consumeSse(stream: ReadableStream<Uint8Array>, reqId: unknown): Promise<void> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buf = "";
  let anyWritten = false;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      // SSE 事件以 \n\n 分隔
      let idx: number;
      while ((idx = buf.indexOf("\n\n")) !== -1) {
        const event = buf.slice(0, idx);
        buf = buf.slice(idx + 2);
        const dataLine = extractDataLine(event);
        if (dataLine === null) continue;
        if (dataLine === "[DONE]") continue;
        process.stdout.write(dataLine + "\n");
        anyWritten = true;
      }
    }
    // 处理结尾残余
    if (buf.trim().length > 0) {
      const dataLine = extractDataLine(buf);
      if (dataLine && dataLine !== "[DONE]") {
        process.stdout.write(dataLine + "\n");
        anyWritten = true;
      }
    }
  } finally {
    reader.releaseLock();
  }
  // 有些 SSE 实现只发事件，不返回响应消息体（如通知/进度）。
  // 如果客户端发的是 request（有 id）但服务端没返回响应，补一个超时错。
  // 但请求无 id（notification）就不补。
  // 不在这里做超时处理，靠外层 AbortSignal.timeout。
  void anyWritten;
}

function extractDataLine(eventBlock: string): string | null {
  // 多行 data: 合并（按 SSE 规范多行 data 用 \n 拼接）
  const datas: string[] = [];
  for (const line of eventBlock.split("\n")) {
    if (line.startsWith(":")) continue; // 注释行
    if (line.startsWith("data:")) {
      datas.push(line.slice(5).trimStart());
    }
  }
  if (datas.length === 0) return null;
  return datas.join("\n");
}

export async function connect(): Promise<void> {
  const url = process.env["MS_MANAGE_URL"];
  if (!url) fatal("MS_MANAGE_URL env is required (e.g. http://127.0.0.1:39231/mcp)");

  if (!process.stdin.isTTY) {
    process.stdin.setEncoding("utf8");
  }

  debug("starting", { url, hasToken: !!process.env["MS_MANAGE_TOKEN"] });

  let inFlight = 0;
  let closed = false;
  const finished = new Promise<void>((resolve) => {
    const check = () => {
      if (closed && inFlight === 0) resolve();
    };
    process.stdin.on("close", () => {
      closed = true;
      debug("stdin closed, draining in-flight requests", { inFlight });
      check();
    });
    process.stdin.on("end", () => {
      closed = true;
      check();
    });
    process.stdin.on("error", (e) => {
      process.stderr.write(`[ms-manage] stdin error: ${e.message}\n`);
      closed = true;
      check();
    });
    // 兜底：每 100ms 检查一次
    const t = setInterval(() => {
      if (closed && inFlight === 0) {
        clearInterval(t);
        resolve();
      }
    }, 100);
  });

  let buf = "";
  process.stdin.on("data", (chunk: string | Buffer) => {
    buf += typeof chunk === "string" ? chunk : chunk.toString("utf8");
    let idx: number;
    while ((idx = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, idx);
      buf = buf.slice(idx + 1);
      const trimmed = line.trim();
      if (!trimmed) continue;
      inFlight++;
      handleOne(trimmed).catch((e) => {
        debug("unhandled error", e?.message ?? String(e));
      }).finally(() => {
        inFlight--;
      });
    }
  });

  // 进程保活
  await finished;
  process.exit(0);
}