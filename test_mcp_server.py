#!/usr/bin/env python3
"""MCP test server for Fe integration testing.
Implements JSON-RPC 2.0 over stdio with tools, resources, and prompts."""

import sys
import json
import uuid


def make_response(id, result=None, error=None):
    resp = {"jsonrpc": "2.0", "id": id}
    if error:
        resp["error"] = {"code": error[0], "message": error[1]}
    else:
        resp["result"] = result
    return resp


def handle_request(req):
    req_id = req.get("id", uuid.uuid4().int & 0x7FFFFFFFFFFFFFFF)
    method = req.get("method", "")
    params = req.get("params", {})

    if method == "initialize":
        return make_response(req_id, {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {"listAvailable": True},
                "resources": {"listAvailable": True},
                "prompts": {"listAvailable": True}
            },
            "serverInfo": {
                "name": "test-mcp-server",
                "version": "0.1.0"
            }
        })

    elif method == "tools/list":
        return make_response(req_id, {
            "tools": [
                {
                    "name": "echo",
                    "description": "Echo back the input message",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "message": {
                                "type": "string",
                                "description": "Message to echo"
                            }
                        },
                        "required": ["message"]
                    }
                },
                {
                    "name": "ping",
                    "description": "Returns pong",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                },
                {
                    "name": "greet",
                    "description": "Greet a person by name",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Name to greet"
                            }
                        },
                        "required": ["name"]
                    }
                }
            ]
        })

    elif method == "tools/call":
        tool_name = params.get("name", "")
        arguments = params.get("arguments", {})

        if tool_name == "echo":
            msg = arguments.get("message", "")
            return make_response(req_id, {
                "content": [
                    {
                        "type": "text",
                        "text": f"Echo: {msg}"
                    }
                ]
            })
        elif tool_name == "ping":
            return make_response(req_id, {
                "content": [
                    {
                        "type": "text",
                        "text": "pong"
                    }
                ]
            })
        elif tool_name == "greet":
            name = arguments.get("name", "World")
            return make_response(req_id, {
                "content": [
                    {
                        "type": "text",
                        "text": f"Hello, {name}!"
                    }
                ]
            })
        else:
            return make_response(req_id, error=(-32601, f"Tool not found: {tool_name}"))

    elif method == "resources/list":
        return make_response(req_id, {
            "resources": [
                {
                    "uri": "greeting://hello",
                    "name": "Hello Resource",
                    "description": "A simple greeting resource",
                    "mimeType": "text/plain"
                },
                {
                    "uri": "info://version",
                    "name": "Version Info",
                    "description": "Server version information",
                    "mimeType": "application/json"
                }
            ]
        })

    elif method == "resources/read":
        uri = params.get("uri", "")
        if uri == "greeting://hello":
            return make_response(req_id, {
                "contents": [
                    {
                        "uri": uri,
                        "mimeType": "text/plain",
                        "text": "Hello from MCP test server!"
                    }
                ]
            })
        elif uri == "info://version":
            return make_response(req_id, {
                "contents": [
                    {
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": '{"name": "test-mcp-server", "version": "0.1.0", "language": "python"}'
                    }
                ]
            })
        else:
            return make_response(req_id, error=(-32602, f"Resource not found: {uri}"))

    elif method == "prompts/list":
        return make_response(req_id, {
            "prompts": [
                {
                    "name": "say_hello",
                    "description": "Generate a greeting message",
                    "arguments": [
                        {
                            "name": "name",
                            "description": "Name to greet",
                            "required": True
                        },
                        {
                            "name": "language",
                            "description": "Language (es/en)",
                            "required": False
                        }
                    ]
                },
                {
                    "name": "show_info",
                    "description": "Show server information",
                    "arguments": []
                }
            ]
        })

    elif method == "prompts/get":
        prompt_name = params.get("name", "")
        arguments = params.get("arguments", {})

        if prompt_name == "say_hello":
            name = arguments.get("name", "World")
            lang = arguments.get("language", "en")
            if lang == "es":
                text = f"¡Hola, {name}! Bienvenido al servidor MCP de prueba."
            else:
                text = f"Hello, {name}! Welcome to the MCP test server."
            return make_response(req_id, {
                "messages": [
                    {
                        "role": "assistant",
                        "content": {
                            "type": "text",
                            "text": text
                        }
                    }
                ]
            })
        elif prompt_name == "show_info":
            return make_response(req_id, {
                "messages": [
                    {
                        "role": "assistant",
                        "content": {
                            "type": "text",
                            "text": "MCP Test Server v0.1.0 — Python implementation for Fe integration testing."
                        }
                    }
                ]
            })
        else:
            return make_response(req_id, error=(-32602, f"Prompt not found: {prompt_name}"))

    elif method == "shutdown":
        return make_response(req_id, None)

    else:
        return make_response(req_id, error=(-32601, f"Method not found: {method}"))


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
            response = handle_request(request)
            sys.stdout.write(json.dumps(response) + "\n")
            sys.stdout.flush()
        except json.JSONDecodeError as e:
            error_resp = {
                "jsonrpc": "2.0",
                "id": None,
                "error": {"code": -32700, "message": f"Parse error: {e}"}
            }
            sys.stdout.write(json.dumps(error_resp) + "\n")
            sys.stdout.flush()


if __name__ == "__main__":
    main()
