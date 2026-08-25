<p align="center">
  <img src="assets/icon-transparent.png" alt="Locoryn icon" width="180">
</p>

<h1 align="center">Locoryn</h1>

<p align="center">
  A desktop interface for local AI models with web search, reasoning, image support, and more.
</p>

<p align="center">
  <a href="https://github.com/logancammish/locoryn/actions/workflows/rust.yml"><img src="https://github.com/logancammish/locoryn/actions/workflows/rust.yml/badge.svg" alt="Build"></a>
  <a href="https://github.com/logancammish/locoryn/releases/latest"><img src="https://img.shields.io/github/v/release/logancammish/locoryn?display_name=tag" alt="Latest release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/logancammish/locoryn" alt="License"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/built_with-Rust-dca282?logo=rust" alt="Rust"></a>
</p>

<p align="center">
  <a href="https://github.com/logancammish/locoryn/releases/latest">Download</a>
  ·
  <a href="https://ollama.com/search">Browse Ollama models</a>
  ·
  <a href="#openvino-setup-linux-and-windows">OpenVINO setup</a>
  ·
  <a href="#build-from-source">Build from source</a>
</p>

<sub>Locoryn is a fork of [ollama-gui-interface](https://github.com/logancammish/ollama-gui-interface).</sub>

<sub><b>What's different?</b></sub>

<sub>
Locoryn builds on ollama-gui-interface, which I originally developed in 2025 as
a Windows-first, Linux-compatible and x86-exclusive application for running 
Ollama models locally. However, as the project expanded I realised that limiting
it to the "ollama-gui-interface" branding was holding it back; people don't 
exclusively use Ollama to host applications, and the project was quickly becoming 
very different to what I orignally intended.
</sub>

<sub>
This application is different. It has different goals, and a different approach.
Now supporting macOS, arm, and with a now Linux-first, Windows-compatible design
approach, it not longer just sticks with Ollama. I hope you will have a positive
experience with it!
</sub>
<br>
<br>


> [!NOTE]
> This README describes the current `main` branch (`1.2.1-pre3`). Packaged releases
> may trail the source branch; check the release notes for the exact feature set
> in a download.

## Why this app?

Locoryn is for people who like running open models locally but want
more control than a basic chat window provides. It keeps the everyday workflow
simple while putting advanced controls—remote hosts, reasoning effort, system
prompts, token limits, streaming behaviour, storage, and filtering—within easy
reach.

It is a particularly good fit if you want to:

- choose between Ollama and OpenVINO Model Server (OVMS) without changing the
  chat workflow;
- connect to an inference server on another computer instead of only
  `localhost`;
- choose exactly how much supported models reason;
- separate disposable chats from conversations worth keeping;
- inspect images with vision-capable models;
- give tool-capable models optional, source-linked access to the web; or
- tune generation and rendering without building your own inference client.

## Quick start

### 1. Start an inference backend

Ollama remains the default. Download and start
[Ollama](https://ollama.com/download). On Linux, make sure its service is
running:

```bash
ollama serve
```

To use Intel CPU, GPU, or NPU acceleration instead, start OpenVINO Model Server
and follow the [Linux and Windows OpenVINO setup](#openvino-setup-linux-and-windows).

### 2. Install Locoryn

- **Windows:** download and run
  `locoryn-1.2.1-pre3-windows-11-x64-setup.exe` from the
  [latest release](https://github.com/logancammish/locoryn/releases/latest).
  It installs for the current user and does not require administrator access.
- **Linux:** run these commands in a terminal. The installer downloads the
  right published package, installs it for the current user, and adds it to
  your desktop launcher. It does not use `sudo`.

  ```sh
  git clone --depth 1 https://github.com/logancammish/locoryn.git
  cd locoryn
  sh install-linux.sh
  ```

  To choose a beta build or architecture manually, see the
  [Linux installer options](linux_installations/README.md).

### 3. Open Locoryn

Launch Locoryn from the Start menu or application launcher. The model picker
lists models available from the selected server.

- For Ollama, an empty list can be filled from **Settings → Advanced settings →
  Install model**. Find model names in the
  [Ollama library](https://ollama.com/search).
- For OpenVINO, open **Settings → Advanced settings**, select **OpenVINO**, and
  enter the OVMS address. The default is `http://127.0.0.1:8000`. Models are
  deployed on OVMS and discovered automatically through `/v3/models`.

## OpenVINO setup (Linux and Windows)

Locoryn's OpenVINO integration is machine agnostic. The desktop app does not
link to the OpenVINO runtime, inspect the local processor, or assume an
instruction-set architecture. It uses OVMS's OpenAI-compatible HTTP endpoints:
`/v3/models` for discovery and `/v3/chat/completions` for generation. OVMS may
run on the same machine or any reachable Linux or Windows host; that server
chooses the Intel CPU, GPU, NPU, or heterogeneous device configuration.

The following small CPU example is based on the
[official OVMS serving guide](https://docs.openvino.ai/2026/model-server/ovms_docs_serving_model.html).
It is a portable starting point; choose a larger compatible model when the
server has enough memory.

First download the example model on the server host:

```bash
python -m pip install huggingface_hub
hf download OpenVINO/Qwen3-0.6B-int4-ov --local-dir Qwen3-0.6B-int4-ov
```

### Linux server

With Docker installed, run:

```bash
docker run -d --rm \
  -v "$PWD/Qwen3-0.6B-int4-ov:/model" \
  -p 8000:8000 \
  openvino/model_server:latest \
  --model_path /model \
  --model_name qwen3-0.6 \
  --rest_port 8000 \
  --task text_generation \
  --target_device CPU \
  --tool_parser hermes3 \
  --reasoning_parser qwen3
```

### Windows server

Install and unpack the current OVMS Windows binary package using the
[official bare-metal guide](https://docs.openvino.ai/2026/model-server/ovms_docs_deploying_server_baremetal.html),
run its `setupvars.bat` or `setupvars.ps1` in each new shell, then run:

```powershell
ovms.exe --model_path Qwen3-0.6B-int4-ov `
  --model_name qwen3-0.6 `
  --rest_port 8000 `
  --task text_generation `
  --target_device CPU `
  --tool_parser hermes3 `
  --reasoning_parser qwen3
```

Check that the server exposes the model:

```bash
curl http://127.0.0.1:8000/v3/models
```

Then select **OpenVINO** under **Settings → Advanced settings → Inference
backend**. Keep the default address for a server on the same machine, or enter
the reachable hostname/IP and REST port of a remote OVMS host. Locoryn retains
separate addresses for Ollama and OpenVINO when you switch between them.

The CPU example works without accelerator-specific container mappings. To use
an Intel GPU or NPU, follow the
[OVMS accelerator guide](https://docs.openvino.ai/2026/model-server/ovms_docs_target_devices.html)
for the required driver, `--target_device`, image, and host-device settings.
Reasoning, tool calls, and image input also depend on the deployed model and its
OVMS parser/pipeline configuration.

### Prefer the simpler classic version?

If you want a smaller, less feature-heavy interface that feels more like a
simple desktop project, try
[version 0.3.7](https://github.com/logancammish/ollama-gui-interface/releases/tag/0.3.7).
It keeps the experience more basic and may suit users who do not need the newer
web, image, storage, and workspace features.

> [!WARNING]
> **Version 0.3.7 is no longer supported or maintained.** It does not receive
> bug fixes, security updates, compatibility work, or help with new Ollama
> releases. Use the latest release for the supported experience.

## Configure the experience

Most controls live in **Settings**:

| Setting | What it controls |
|---|---|
| Model and reasoning | Active backend model and its supported thinking effort |
| Response and context limits | Output cap and how much conversation the model can hold |
| Temperature | Predictability versus variety |
| System prompt | Active instruction/personality profile |
| Dynamic system prompt | Date, time, user name, and custom per-request instructions |
| Web search | Provider, API key, results per search, request timeout, deep-research budgets, source checkpoints, and custom research guidance |
| Application updates | Current version, latest stable release, and a trusted download link |
| Chat storage | The folder containing saved conversations |
| Model conversation context | Whether earlier messages are included in the next request |
| Interface | Language, theme, text size, and chat font |

**Advanced settings** contains backend selection, separate Ollama and OpenVINO
connection details, Ollama model installation, streaming/batching controls,
content filtering, the local code checker, and password protection.

### Settings password protection

Educational and shared-device deployments can set a password under **Settings
→ Advanced settings → Password protection**. Advanced settings are always
covered when protection is enabled; **Protection coverage** can extend the lock
to the standard Settings page and its compact chat configuration controls.
Choose the coverage first, then select **Save and enable**. Locoryn immediately
writes the complete password policy and shows the lock screen so the new
password can be verified. Leaving the protected settings area locks it again.

The configuration is deliberately reproducible: `password_enabled`, `password`,
and `password_scope` are ordinary top-level values in the local `settings.json`.
The password is stored as plaintext, so copying the same settings file to
another installation reproduces the same policy. A missing `password_scope`
defaults to `all_settings`; the other supported value is
`advanced_settings_only`.

> [!WARNING]
> Not intended for high vunerability environments

### Local code checking

Code checking is disabled by default. To let a tool-capable model check its own
snippets, enable both **Settings → Tools → Code Checking** and **Settings →
Advanced settings → Local code checking**. The advanced setting also enables
the manual **Check code** button on supported fenced code blocks. The app uses `python3 -m
py_compile`, `rustc`, `cc`, `c++`, or `csc` in a temporary folder and reports
the first errors without running the compiled program.

Generated code is untrusted input. Compiler and interpreter checks can fail,
consume resources, or have unintended effects, so enable this only when you
consent and have reviewed the code. Checks are bounded and only accept
self-contained snippets; they do not run the program.

### Custom system prompts

Prompt profiles come from [`config/defaultprompts.json`](config/defaultprompts.json).
Add a JSON key/value pair where the key is the profile name and the value is the
instruction, then restart the application:

```json
{
  "concise": "Answer directly. Prefer short explanations and concrete examples.",
  "reviewer": "Review the supplied code for correctness, security, and maintainability."
}
```

Keep the file as valid JSON. When using an installed build, edit the copy in the
`config` folder beside the executable.

### Remote inference servers

Open **Settings → Advanced settings**, select the backend, and choose the
protocol before entering its hostname/IP and port. The protocol field accepts
`http`, `http://`, `https`, or `https://`. The defaults are
`http://127.0.0.1:11434` for Ollama and `http://127.0.0.1:8000` for OpenVINO.
Each backend keeps its own saved address. Other URL schemes are rejected
because both APIs use HTTP(S).

Use `https://` for any inference server reached over a network you do not fully
control. Plain `http://` does not encrypt requests: prompts, model replies,
attached images, and API responses can be read or changed by someone able to
observe the connection. HTTP also does not authenticate the server, so it is
vulnerable to impersonation on an untrusted network.

Do not expose Ollama or OVMS directly to the public internet. Put a remote
service behind a correctly configured TLS reverse proxy or VPN, restrict who
can reach it, and use a certificate trusted by the computer running Locoryn.
Locoryn does not currently add authentication headers to inference requests;
if your deployment requires authentication, enforce access at the network or
proxy layer and allow Locoryn only from trusted clients.

### Web search setup

Web search requires a model that supports tool calling and an API key from one
of the supported providers. For OpenVINO, deploy the model with the appropriate
OVMS tool parser:

| Provider | Status | Preferred environment variable |
|---|---|---|
| [Brave Search](https://brave.com/search/api/) | Stable, default | `BRAVE_SEARCH_API_KEY` |
| [Tavily](https://docs.tavily.com/documentation/api-reference/endpoint/search) | Experimental | `TAVILY_API_KEY` |
| [Exa](https://exa.ai/docs/reference/search) | Experimental | `EXA_API_KEY` |

Tavily and Exa support may change as their APIs evolve. Their current adapters
use the provider's standard search endpoint, map the app's freshness choices,
and keep webpage fetching behind the same public-network safety checks as Brave.

1. Open **Settings** and enable **Web Search**.
2. Choose a search provider and result limit. Leave **Brave Search** selected
   for the established integration.
3. Optionally enable **Deep follow-up research**, then tune maximum searches,
   required successful searches, maximum page reads, required independent
   pages, maximum tool rounds, and custom research instructions.
4. Provide the API key using one of the methods below.
5. Use the **Web** button beside the chat input whenever you want web tools
   enabled for that conversation.

The preferred approach is to set the selected provider's key before launching
the app. For example:

```bash
export BRAVE_SEARCH_API_KEY="your-key"
```

Use `TAVILY_API_KEY` or `EXA_API_KEY` instead when selecting an experimental
provider. You can also enter keys in Settings. Each provider keeps a separate
saved key in the local `settings.json`; API keys are redacted from diagnostics
and are not printed in logs.

> [!IMPORTANT]
> Ordinary chats are sent only to the inference backend address you configure.
> When web search is enabled, search queries are also sent to the selected
> search provider and the app fetches public webpages selected by the model. A
> remote inference server receives the conversation data needed to answer your
> request. The update manager sends a version-check request to GitHub at startup
> and when you select **Check now**.

## Local data and privacy

Chats, settings, and diagnostics are stored on your machine.
Temporary chats are not added to `chats.json`.

| Platform | Default application-data folder |
|---|---|
| Windows | `%LOCALAPPDATA%\Ollama GUI` |
| Linux | `$XDG_DATA_HOME/locoryn` or `~/.local/share/locoryn` |

Saved conversations live in the `chats` subfolder by default. The exact active
path is always visible under **Settings → Chat storage**, and it can be changed
from there. The application can also detect conversations from the legacy
`output/chats.json` location.

User preferences use `settings.json` in the application-data folder.

## Platform support

| Platform | Status |
|---|---|
| Windows x64 | Officially supported; per-user installer available |
| Windows ARM64 | Native portable build available |
| Linux x86_64 and ARM64 | Officially supported; desktop installer available |
| macOS Apple Silicon (ARM64) | Build supported; automated native build available |

Either Ollama or an OpenVINO Model Server must be running locally or reachable
at its configured address. OVMS itself has its own supported host and hardware
requirements; the Locoryn client does not need to run on the same operating
system or processor architecture as the inference server. Rust and Cargo are
required only when building Locoryn from source.

## Build from source

Install the [Rust toolchain](https://rustup.rs/), then:

```bash
git clone https://github.com/logancammish/locoryn.git
cd locoryn
cargo build --release
```

The finished binary is `target/release/locoryn` on Linux and macOS, and
`target\release\locoryn.exe` on Windows. On an Apple Silicon Mac, build it
natively with:

```bash
cargo build --release --target aarch64-apple-darwin
```

On Linux, build Linux x86_64/ARM64 and Windows x86_64/ARM64 together with all
available CPU cores by running:

```bash
./linux_installations/build-locally.sh --install-tools
```

See [the Linux build documentation](linux_installations/README.md#build-every-desktop-target-locally)
for prerequisites and output locations.

The macOS ARM64 binary is then
`target/aarch64-apple-darwin/release/locoryn`.

Run a development build with:

```bash
cargo run
```

Before submitting a change, run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build
```

## Project links

- [Download the latest release](https://github.com/logancammish/locoryn/releases/latest)
- [Browse Ollama models](https://ollama.com/search)
- [Install Ollama](https://ollama.com/download)
- [Deploy OpenVINO Model Server](https://docs.openvino.ai/2026/model-server/ovms_docs_deploying_server.html)
- [Browse OpenVINO LLM models](https://huggingface.co/collections/OpenVINO/llm)
- [Read the OVMS OpenAI-compatible API reference](https://docs.openvino.ai/2026/model-server/ovms_docs_rest_api_chat.html)
- [View the source repository](https://github.com/logancammish/locoryn)
- [Read the GNU GPL v3 license](LICENSE)

---

Locoryn is an independent open-source project built for people who want a
configurable, local-first desktop experience with their preferred inference
backend.
