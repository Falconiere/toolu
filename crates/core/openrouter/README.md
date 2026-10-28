# OpenRouter client

`toolu-openrouter` is the shared core client for OpenRouter. It uses `toolu-http`
for bounded HTTPS and takes credentials only from the supplied `Env`:
`OPENROUTER_API_KEY` is required, and `OPENROUTER_MODEL` can override the pinned
`deepseek/deepseek-v4-pro-0813` default. It never reads a repository `.env` file.

`Client::complete_structured` accepts a caller deadline and a JSON schema. It
sends a strict non-streaming chat request with explicit reasoning effort and
`reasoning.exclude: true`. Provider routing requires parameter support, zero
data retention, and fp8/bf16/fp16 quantization. It does not retry without those
constraints. All attempts and waits share the same deadline; responses are
limited to 1 MiB. Authentication, rate limit, no eligible provider, timeout,
transport, empty content, and reasoning-only truncation are typed errors. Error
text never includes the key, prompt, or upstream body.

A successful `Completion` records the requested model, serving provider when
OpenRouter supplies one, finish reason, token counts, and cost. Missing cost or
provider stays unknown. The code-review crate owns the findings schema and
local validation; future streaming and tool calls extend this shared crate.
