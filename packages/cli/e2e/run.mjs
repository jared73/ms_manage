// 端到端 smoke test：起一个 mock HTTP MCP server，跑 CLI 连过去，
// 喂 initialize + tools/list 两个请求，看 stdout 是不是两个对应的 JSON-RPC 响应。
//
// 用法：node packages/cli/e2e/run.mjs
import http from "node:http";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const cliPath = resolve(__dirname, "../dist/index.js");

const server = http.createServer((req, res) => {
  let body = "";
  req.on("data", (c) => (body += c.toString("utf8")));
  req.on("end", () => {
    let reqJson;
    try { reqJson = JSON.parse(body); } catch { res.writeHead(400).end(); return; }
    const id = reqJson.id;
    if (reqJson.method === "initialize") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({
        jsonrpc: "2.0",
        id,
        result: {
          protocolVersion: "2024-11-05",
          capabilities: { tools: { listChanged: false } },
          serverInfo: { name: "mock-ms-manage", version: "0.0.0" },
        },
      }));
      return;
    }
    if (reqJson.method === "tools/list") {
      res.writeHead(200, { "Content-Type": "text/event-stream", "Cache-Control": "no-cache" });
      res.write(`event: message\n`);
      res.write(`data: ${JSON.stringify({
        jsonrpc: "2.0",
        id,
        result: {
          tools: [
            { name: "echo", description: "echo back input", inputSchema: { type: "object" } },
          ],
        },
      })}\n\n`);
      res.end();
      return;
    }
    res.writeHead(404).end();
  });
});

server.listen(0, "127.0.0.1", () => {
  const port = server.address().port;
  console.log(`mock MCP server listening on http://127.0.0.1:${port}/mcp`);

  const cli = spawn(process.execPath, [cliPath], {
    env: {
      ...process.env,
      MS_MANAGE_URL: `http://127.0.0.1:${port}/mcp`,
      MS_MANAGE_TOKEN: "test-token-abc",
      MS_MANAGE_DEBUG: "1",
    },
  });

  let stderr = "";
  cli.stderr.on("data", (c) => (stderr += c.toString("utf8")));

  let stdout = "";
  cli.stdout.on("data", (c) => (stdout += c.toString("utf8")));

  // 1) initialize
  cli.stdin.write(JSON.stringify({
    jsonrpc: "2.0", id: 1, method: "initialize",
    params: { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "test", version: "0" } },
  }) + "\n");
  // 2) notifications/initialized （无 id，服务端不响应）
  cli.stdin.write(JSON.stringify({
    jsonrpc: "2.0", method: "notifications/initialized",
  }) + "\n");
  // 3) tools/list
  cli.stdin.write(JSON.stringify({
    jsonrpc: "2.0", id: 2, method: "tools/list", params: {},
  }) + "\n");

  // 等响应回来后关 stdin（不要等超时）
  setTimeout(() => cli.stdin.end(), 500);

  cli.on("close", (code) => {
    console.log(`\nCLI exit code: ${code}`);
    console.log(`--- stdout ---\n${stdout}`);
    if (stderr) console.log(`--- stderr ---\n${stderr}`);

    // 简单校验
    const lines = stdout.split("\n").filter((l) => l.trim().length > 0).map((l) => JSON.parse(l));
    const init = lines.find((m) => m.id === 1);
    const toolsList = lines.find((m) => m.id === 2);
    let ok = true;
    if (!init || init.result?.serverInfo?.name !== "mock-ms-manage") {
      console.error("✗ initialize response missing/wrong"); ok = false;
    } else console.log("✓ initialize response OK");
    if (!toolsList || !Array.isArray(toolsList.result?.tools) || toolsList.result.tools[0]?.name !== "echo") {
      console.error("✗ tools/list response missing/wrong"); ok = false;
    } else console.log("✓ tools/list response OK (SSE → newline-delimited JSON)");

    server.close();
    process.exit(ok ? 0 : 1);
  });
});