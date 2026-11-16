# Rust rewrite architecture

This repository mirrors the Python project's chapter-by-chapter teaching flow.
Each `src/bin/chN_MM_*.rs` is a runnable lesson; shared code lives in `src/` so
provider configuration, transport setup, and example servers do not drift.

```text
chapter binary
  ├── config.rs ── dotenv + ANTHROPIC_API_KEY / ANTHROPIC_BASE_URL / model
  ├── Rig Anthropic provider ── model request and response types
  ├── handwritten host loop ── history, tool schemas, tool results
  └── rmcp client ── stdio / Streamable HTTP ── rmcp server
```

## Design choices

- Rig is the provider adapter. Its Anthropic client is created only in
  `config::anthropic_client()`, so API key and compatible gateway URL come from
  environment variables instead of chapter files.
- The agent loop stays visible in the relevant chapter binary. Rig's low-level
  completion request API carries messages and tool definitions; the example
  dispatches each model tool call through `rmcp` and appends the result.
- `rmcp` is the MCP protocol implementation on both sides. Chapter servers use
  its `ServerHandler` APIs or tool macros; chapter hosts use its typed request
  and transport APIs.
- Each lesson is an independent Cargo binary. A binary that needs an in-process
  demo server can start itself with `--server`, keeping the client and server
  protocol boundary real while avoiding external setup.

## Current implementation map

| Python chapter area | Rust entry points / modules | Coverage |
| --- | --- | --- |
| ch3: model-only host and client interface | `ch3_01_host_no_client`, `ch3_02_host_with_client_interface` | Stateless interactive host using the shared provider configuration; mirrors the source's early client-interface stub |
| ch3: stdio client lifecycle | `ch3_03_stdio_client_constructor`, `ch3_04_connect_disconnect_stdio`, `ch3_07_mcpclient_context_manager`, `ch3_08_stdio_client` | Construction, use, and explicit disconnect are shown across short-lived and wrapped clients |
| ch3: tool listing, invocation, and progress | `ch3_09_wrap_list_tools`, `ch3_10_implement_use_tool`, `ch3_11_receive_progress_notification`, `ch3_12_make_tool_calls`, `calculator_server` | Lists and calls tools, reports progress, and hands model tool calls to the MCP server |
| ch3: resources and prompts | `ch3_13`–`ch3_18`, `ch3_content_server`, `agent_loop` | Source-aligned `math-constants` resource and `calculate_operation` prompt; `ch3_15` selects relevant resources, and `ch3_15`/`ch3_18` run the calculator tool loop |
| ch3: Streamable HTTP transport | `ch3_05_streamable_http_client_constructor`, `ch3_06_streamable_http_connect_disconnect`, `ch3_http_demo_server` | Connects to a configurable remote endpoint; local demo server supports the constructor and connect/disconnect examples |
| ch4: models and multiple MCP servers | `ch4_05_multiple_models`, `ch4_07_multiple_servers` | `ch4_05` runs a draft/review pair and exercises calculator tools, resource selection, prompts, sampling, roots, elicitation, logging, and dynamic `bonus_tool` registration; `ch4_07` selects resources/prompts from separate calculator and content servers and dispatches namespaced tools |
| ch4: registry, per-turn tools, search, subscriptions | `ch4_06_registry_resolution`, `ch4_08_per_turn_tool_assembly`, `ch4_09_tool_search`, `ch4_10_recovering_subscriptions` | Registry resolution is opt-in for package execution; ch4.8 and ch4.9 refresh tools per turn and support resource/prompt flows; search reveals matching tools for that turn; ch4.10's tool-list notification invalidates the cache before the next turn |
| ch4: sampling, roots, elicitation, OAuth | `ch4_01`–`ch4_04`, `ch6_servers`, `tutorial_servers` | `ch4_01`–`ch4_03` run the Python-style interactive calculator agent with progressively enabled sampling, roots, and elicitation callbacks; OAuth authorization code + PKCE |
| ch5: MCP server tools/resources/prompts | `ch5_01`–`ch5_16`, `tutorial_servers`, `calculator_server`, `content_server` | Explicit low-level tool listing/calls and structured output, per-example server metadata, lifespan state, typed report card with PNG image output, tool-level grade lookup errors, prompt examples, bundled resources, and bounded file-backed log slices |
| ch6: completions, icons, context, logging, progress, notifications, pagination, elicitation, sampling, roots, cancellation | `ch6_01`–`ch6_12`, `ch6_servers`, `completion_server`, `tutorial_servers` | Source-specific prompt/resource completions, icon metadata, session resource updates, dynamic prompt notifications, length-based progress, model preferences, and cancellation; compile plus stdio behavior checks |
| ch7: tool evaluation | `ch7_01_simple_evaluation` | Four-case tool-choice score, matching the source evaluation |
| ch7: HTTP client, OAuth-protected server, and deployment | `ch7_02`–`ch7_06`, `Dockerfile` | Streamable HTTP client, mounted stateless service, and container image; chapter 7.3 uses a static bearer-token check as a runnable stand-in for the source's unfinished verifier |

The examples are intentionally progressive: later lessons build on these
shared primitives while keeping each lesson runnable on its own. Sampling,
roots, and logging are deprecated by the pinned MCP SDK version's newer
specification, but their examples remain because they are part of the source
curriculum.
