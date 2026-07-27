<p align="center">
  <img src="assets/icon.png" alt="Locoryn icon" width="180">
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
  <a href="#build-from-source">Build from source</a>
</p>

Locoryn is a fork of [ollama-gui-interface](https://github.com/logancammish/ollama-gui-interface).

> [!NOTE]
> This README describes the current `main` branch (`1.0.0`). Packaged releases
> may trail the source branch; check the release notes for the exact feature set
> in a download.

## Why this app?

Locoryn is for people who like running open models locally but want
more control than a basic chat window provides. It keeps the everyday workflow
simple while putting advanced controls—remote hosts, reasoning effort, system
prompts, token limits, streaming behaviour, storage, and filtering—within easy
reach.

It is a particularly good fit if you want to:

- connect to Ollama on another computer instead of only `localhost`;
- choose exactly how much supported models reason;
- separate disposable chats from conversations worth keeping;
- inspect images or experiment with Ollama image-generation models;
- give tool-capable models optional, source-linked access to the web; or
- tune generation and rendering without building your own Ollama client.

## Quick start

### 1. Install Ollama

Download and start [Ollama](https://ollama.com/download). On Linux, make sure its
service is running:

```bash
ollama serve
```

### 2. Install Locoryn

Open the [latest release](https://github.com/logancammish/locoryn/releases/latest)
and choose the asset for your operating system.

- **Windows 11:** use `locoryn-1.0.0-windows-11-x64-setup.exe` for the standard per-user
  installation. It does not require administrator privileges.
- **Linux:** download the Linux archive when one is provided, extract it, and
  run the `locoryn` executable.

### 3. Choose a model and chat

The model picker automatically lists models available from the connected Ollama
server. If the list is empty, open **Settings → Advanced settings**, enter a
model name under **Install model**, and press Enter. Find model names in the
[Ollama library](https://ollama.com/search).

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
| Model and reasoning | Active Ollama model and its supported thinking effort |
| Response and context limits | Output cap and how much conversation the model can hold |
| Temperature | Predictability versus variety |
| System prompt | Active instruction/personality profile |
| Dynamic system prompt | Date, time, user name, and custom per-request instructions |
| Web search | Provider, API key, results per search, request timeout, deep-research budgets, source checkpoints, and custom research guidance |
| Application updates | Current version, latest stable release, and a trusted download link |
| Chat storage | The folder containing saved conversations |
| Model conversation context | Whether earlier messages are included in the next request |
| Interface | Language, theme, and text size |

**Advanced settings** contains model installation, custom Ollama connection
details, streaming/batching controls, content filtering, and the local code
checker.

### Local code checking

Code checking is disabled by default. Read the warning and explicitly enable it
under **Settings → Advanced settings → Local code checking**. Supported fenced
code blocks then show a **Check code** button. The app uses `python3 -m
py_compile`, `rustc`, `cc`, `c++`, or `csc` in a temporary folder and reports
the first errors without running the compiled program.

Generated code is untrusted input. Compiler and interpreter checks can fail,
consume resources, or have unintended effects, so enable this only when you
consent and have reviewed the code.

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

### Remote Ollama servers

Open **Settings → Advanced settings → Ollama address** and enter the server host
or IP plus its port. The default is `127.0.0.1:11434`.

The application currently connects over HTTP, so only use a trusted network or
put appropriate transport security in front of a remote Ollama instance.

### Web search setup

Web search requires a model that supports Ollama tool calling and an API key
from one of the supported providers:

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
> Ordinary chats are sent only to the Ollama address you configure. When web
> search is enabled, search queries are also sent to the selected search
> provider and the app fetches public webpages selected by the model. A remote
> Ollama server receives the conversation data needed to answer your request.
> The update manager sends a version-check request to GitHub at startup and when
> you select **Check now**.

## Local data and privacy

Chats, settings, diagnostics, and generated images are stored on your machine.
Temporary chats are not added to `chats.json`.

| Platform | Default application-data folder |
|---|---|
| Windows | `%LOCALAPPDATA%\Ollama GUI` |
| Linux | `$XDG_DATA_HOME/ollama-gui` or `~/.local/share/ollama-gui` |

Saved conversations live in the `chats` subfolder by default. The exact active
path is always visible under **Settings → Chat storage**, and it can be changed
from there. The application can also detect conversations from the legacy
`output/chats.json` location.

Generated images are stored in `generated`, while user settings and diagnostics
use `settings.json` and `history.json` in the application-data folder.

## Platform support

| Platform | Status |
|---|---|
| Windows x64 | Officially supported; per-user installer available |
| Linux x64 | Officially supported on Wayland |
| macOS Apple Silicon (ARM64) | Build supported; automated native build available |

Ollama itself must be installed and running locally or reachable at the custom
address you configure. Rust and Cargo are required only when building from
source.

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
- [View the source repository](https://github.com/logancammish/locoryn)
- [Read the GNU GPL v3 license](LICENSE)

---

Locoryn is an independent open-source project built for people who want a
configurable, local-first desktop experience around Ollama.
