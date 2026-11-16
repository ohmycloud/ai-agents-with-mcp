# AI Agents with MCP

This is a chapter-by-chapter Rust rewrite of the Python examples. Each lesson
is a standalone Cargo binary. Shared code keeps provider configuration and
example MCP servers in one place.

The model provider is Rig's Anthropic integration. MCP clients and servers use
the official `rmcp` SDK. The Agent/tool loop stays explicit in the relevant
examples so the model-to-MCP handoff is visible.

## Setup

Copy `.env.example` to `.env` and set:

```dotenv
ANTHROPIC_API_KEY=your-key
ANTHROPIC_MODEL=your-model
# Optional for a compatible proxy or gateway:
ANTHROPIC_BASE_URL=https://api.example.com
```

The base URL and API key are read centrally by `src/config.rs`; chapter files do
not construct provider clients with their own credentials.

## Examples

### Chapter 3: Host and MCP client

```sh
cargo run --bin ch3_01_host_no_client
cargo run --bin ch3_02_host_with_client_interface
cargo run --bin ch3_03_stdio_client_constructor
cargo run --bin ch3_04_connect_disconnect_stdio
cargo run --bin ch3_07_mcpclient_context_manager
cargo run --bin ch3_08_stdio_client
cargo run --bin ch3_09_wrap_list_tools
cargo run --bin ch3_10_implement_use_tool
cargo run --bin ch3_11_receive_progress_notification
cargo run --bin ch3_12_make_tool_calls
cargo run --bin ch3_13_wrap_list_resources
cargo run --bin ch3_14_get_resource
cargo run --bin ch3_15_use_resource
cargo run --bin ch3_16_get_available_prompts
cargo run --bin ch3_17_load_prompt
cargo run --bin ch3_18_use_prompt
```

The stdio client examples start their demo server as a child process.
The resource and prompt examples serve the course calculator's
`resource://math-constants` resource and `calculate_operation` prompt. In
`ch3_15` and `ch3_18`, the selected resource or rendered prompt is passed into
the shared explicit MCP calculator tool loop.

For Streamable HTTP, start this server in one terminal:

```sh
cargo run --bin ch3_http_demo_server
```

Then run an HTTP client in another terminal. `MCP_SERVER_URL` is optional and
defaults to `http://127.0.0.1:8000/mcp`.

```sh
cargo run --bin ch3_05_streamable_http_client_constructor
cargo run --bin ch3_06_streamable_http_connect_disconnect
```

### Chapter 4: multiple models and MCP servers

```sh
cargo run --bin ch4_01_sampling_callback
cargo run --bin ch4_02_providing_roots
cargo run --bin ch4_03_returning_elicitations
cargo run --bin ch4_04_authorizing_oauth
cargo run --bin ch4_05_multiple_models
cargo run --bin ch4_06_registry_resolution
cargo run --bin ch4_07_multiple_servers
cargo run --bin ch4_08_per_turn_tool_assembly
cargo run --bin ch4_09_tool_search
cargo run --bin ch4_10_recovering_subscriptions
```

Set `ANTHROPIC_MODEL_FAST` and `ANTHROPIC_MODEL_REVIEW` to choose distinct
models; both fall back to `ANTHROPIC_MODEL`.
`ch4_05` exposes the calculator MCP tools to the fast model's explicit tool
loop, then asks the review model to improve the result. It also demonstrates
the calculator's math resource and prompt, model-based resource selection,
sampling, roots, elicitation, and server log callbacks. Type `refresh` to
reload resources and prompts, or `prompts` to load a server prompt. Optionally
set `ANTHROPIC_MODEL_RESOURCE_SELECTOR` and `ANTHROPIC_MODEL_SAMPLING`; both
fall back to `ANTHROPIC_MODEL`. Asking the agent to install the plugin adds a
`bonus_tool` during the session; the client refreshes the advertised tool list
after that turn. `MCP_ROOT` defaults to the current directory.

`ch4_07_multiple_servers` connects to separate calculator and content servers.
The model chooses relevant resources and prompts from both servers on each
turn, and tool names are namespaced by server. Type `refresh` to reload tools,
resources, and prompts from both connections.

The sampling host needs `ANTHROPIC_API_KEY` and `ANTHROPIC_MODEL`. The roots and
elicitation examples start local stdio servers and invoke their tools so the
server exercises each client callback. Set `MCP_ROOT` to choose the directory
advertised by the roots examples. The elicitation demo accepts when both
`MCP_SIGNUP_NAME` and `MCP_SIGNUP_EMAIL` are set; without them it demonstrates
the decline path. The subscription example starts a local dynamic-tools server
by default; ask it to install the plugin to receive a tool-list change and use
the new `bonus_tool` on the following turn. Set `MCP_SERVER_URL` to connect to
an already-running calculator HTTP server instead. The ch4.8 and ch4.9 agents
also support resource selection, prompt loading, and the `refresh`/`prompts`
commands.

`ch4_04_authorizing_oauth` performs OAuth authorization code + PKCE against the
server at `MCP_SERVER_URL` (default `http://127.0.0.1:8000/mcp`). It prints the
authorization URL and listens on a temporary loopback callback port. Open the
URL, authorize the client, and let the browser return to the callback.

Registry package execution is disabled by default. Set `RUN_REGISTRY_CONNECT=1`
to launch a resolved stdio package with `uvx`. The subscription watcher uses
`MCP_SERVER_URL` and refreshes its tool list after change notifications.


### Chapter 5: MCP servers

```sh
cargo run --bin ch5_01_low_level_server
cargo run --bin ch5_02_low_level_list_call_tools
cargo run --bin ch5_03_low_level_structured_output
cargo run --bin ch5_04_lifespan_management
cargo run --bin ch5_05_minimal_stdio_server
cargo run --bin ch5_06_structured_output
cargo run --bin ch5_07_full_tool_server
cargo run --bin ch5_08_simple_prompt_server
cargo run --bin ch5_09_multiturn_prompt
cargo run --bin ch5_10_tool_use_prompt
cargo run --bin ch5_11_resource_prompt
cargo run --bin ch5_12_basic_resource_server
cargo run --bin ch5_13_resource_template_server
cargo run --bin ch5_14_resource_objects
cargo run --bin ch5_15_handling_errors
cargo run --bin ch5_16_large_resources
```

Chapter 5 binaries are MCP servers over stdio; run them through a compatible
MCP host. The lifespan example records startup, list-tools, tool-call, and
shutdown events. The report-card image example returns a rendered PNG image
block, and the resource-template examples bundle their text and PNG assets in
this crate.

### Chapter 6: server capabilities

```sh
cargo run --bin ch6_01_completions
cargo run --bin ch6_02_server_icons
cargo run --bin ch6_03_context_object_server_info
cargo run --bin ch6_04_context_object_session_info
cargo run --bin ch6_05_context_object_logging
cargo run --bin ch6_06_progress_notification
cargo run --bin ch6_07_manual_notifications
cargo run --bin ch6_08_low_level_pagination
cargo run --bin ch6_09_elicitations_server
cargo run --bin ch6_10_sampling
cargo run --bin ch6_11_roots
cargo run --bin ch6_12_cancel_request_notification
cargo run --bin ch3_11_receive_progress_notification
```

Most Chapter 6 examples are stdio MCP servers. `ch6_12` runs both sides and
cancels a long-running request. The first three Chapter 4 binaries are
interactive calculator agents: `ch4_01` enables sampling, `ch4_02` adds
filesystem roots, and `ch4_03` adds form elicitation. They accept `refresh`,
`prompts`, and `goodbye`, like their Python counterparts.
`ch6_01` demonstrates completion for both prompt arguments and resource URI
templates. `ch6_07` removes a prompt and announces the changed prompt list.
`ch6_04` appends facts to its bundled `knowledge.txt` resource.

### Chapter 7: evaluation and deployment

```sh
cargo run --bin ch7_01_simple_evaluation
cargo run --bin ch7_02_streamable_http_container_client
cargo run --bin ch7_03_mcp_oauth_resource_server
cargo run --bin ch7_04_remote_deployment_prework
cargo run --bin ch7_05_starlette_mount
cargo run --bin ch7_06_remote_deployment_dockerfile
```

The HTTP servers bind to loopback by default, except the container entrypoint,
which binds to `0.0.0.0:8000`. Set `MCP_BIND_ADDRESS` to override the address.
The resource-server example uses a static bearer-token check for the lesson and
expects `MCP_RESOURCE_SERVER_TOKEN`; it does not implement OAuth discovery or
token issuance. Use an external OAuth-enabled MCP server to exercise the
authorization-code client flow in `ch4_04`.

Build and run the container server with:

```sh
docker build -t ai-agents-mcp-rust .
docker run --rm -p 8000:8000 ai-agents-mcp-rust
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design and a current
map of implemented and remaining chapters.
