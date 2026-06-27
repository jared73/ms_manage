"""Minimal MCP server (stdio JSON-RPC) for E2E testing ai-manage proxy.

Implements:
- initialize / notifications/initialized
- tools/list (returns one tool: echo)
- tools/call name=echo (echoes arguments)

Usage: python mock_server.py
"""
import json
import sys


def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def read():
    line = sys.stdin.readline()
    if not line:
        sys.exit(0)
    return json.loads(line)


def main():
    while True:
        msg = read()
        method = msg.get("method")
        id_ = msg.get("id")

        if method == "initialize":
            send({"jsonrpc": "2.0", "id": id_, "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {"listChanged": False}},
                "serverInfo": {"name": "mock-echo", "version": "0.1.0"},
            }})
        elif method == "notifications/initialized":
            pass
        elif method == "tools/list":
            send({"jsonrpc": "2.0", "id": id_, "result": {
                "tools": [{
                    "name": "echo",
                    "description": "Echoes back its input",
                    "inputSchema": {
                        "type": "object",
                        "properties": {"text": {"type": "string"}},
                        "required": ["text"],
                    },
                }]
            }})
        elif method == "tools/call":
            params = msg.get("params", {})
            name = params.get("name")
            args = params.get("arguments", {})
            if name == "echo":
                send({"jsonrpc": "2.0", "id": id_, "result": {
                    "content": [{"type": "text", "text": f"echo: {args.get('text', '')}"}],
                    "isError": False,
                }})
            else:
                send({"jsonrpc": "2.0", "id": id_, "error": {
                    "code": -32601, "message": f"Tool not found: {name}"
                }})
        else:
            if id_ is not None:
                send({"jsonrpc": "2.0", "id": id_, "error": {
                    "code": -32601, "message": f"Method not found: {method}"
                }})


if __name__ == "__main__":
    main()
