# Runbook: llm-step Module

<!--
Canonical Reference: .pi/architecture/modules/llm-step.md
Last Updated: 2026-06-19
-->

## Overview

The `llm-step` module provides a specialized DAG node type (`LlmGenerateNode`)
that wraps LLM calls for code generation and recovery during DAG execution.
It sits between the DAG Engine (which treats it as a regular node) and the
LLM provider (which performs the actual generation).

Key capabilities:
- LLM-based code generation within DAG execution
- Source code context assembly from filesystem
- Failure analysis context for retry augmentation
- Configurable LLM providers (Anthropic, OpenAI, mock)
- Token budget enforcement and retry management

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `LlmGenerateNode` | Domain entity | DAG node: model config, prompt template, output schema, lifecycle state |
| `LlmStepContext` | Domain entity | Source code context + failure analysis assembly |
| `LlmStepServiceImpl` | Application service | Orchestrates context building, LLM generation, retries |
| `LlmContextBuilderServiceImpl` | Application service | Reads source files, formats context, assembles prompts |
| `LlmGenerateNodeRepository` | Repository trait | Node persistence contract |
| `AnthropicProviderClient` | Infrastructure | HTTP client for Anthropic Claude API |
| `OpenAiProviderClient` | Infrastructure | HTTP client for OpenAI Chat Completions API |
| `MockLlmProviderClient` | Infrastructure | Configurable mock for testing |

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tokio runtime | Yes | Async I/O for LLM provider calls and filesystem reads |
| reqwest (json feature) | Yes | HTTP client for LLM provider API calls |
| serde + serde_json | Yes | DTO serialization and LLM response parsing |
| chrono | Yes | Timestamps (ISO 8601 UTC) |
| uuid | Yes | Node and execution identifiers |
| thiserror | Yes | Structured error types |
| async-trait | Yes | Trait object safety for service traits |

### Initialization

1. Create a `LlmContextBuilderServiceImpl` for context assembly (optionally configured with repo root)
2. Create a provider client (`AnthropicProviderClient`, `OpenAiProviderClient`, or `MockLlmProviderClient`)
3. Create a `LlmStepServiceImpl` with the provider client and context builder
4. (Optional) Create an `InMemoryNodeRepository` for node persistence

```rust
use rigorix_engine::llm_step::application::service::*;
use rigorix_engine::llm_step::application::service_impl::*;
use rigorix_engine::llm_step::infrastructure::llm_provider_client_impl::*;

// Create a context builder
let context_builder = LlmContextBuilderServiceImpl::new()
    .with_repo_root("/path/to/repo");

// Create a provider client
let provider_client = AnthropicProviderClient::new(
    "https://api.anthropic.com/v1/messages".to_string(),
    api_key,
    120,
);

// Create the LLM step service
let llm_service = LlmStepServiceImpl::new(
    Box::new(provider_client),
    Box::new(context_builder),
    3,    // max_retries
    120,  // default_timeout_secs
    true, // validate_before_execution
);

// Create a generation node
let node = llm_service.create_node(CreateNodeInput {
    name: "generate-test".to_string(),
    model_config: LlmModelConfig::default(),
    prompt_template: "Generate a test for {source_code}".to_string(),
    output_schema: LlmOutputSchema {
        format: LlmOutputFormat::Code,
        schema: "Generated test code".to_string(),
        strict: false,
    },
}).await.unwrap();

// Execute the full step
let output = llm_service.execute_step(ExecuteStepInput {
    node: node.node,
    execution_id: Uuid::new_v4(),
    dag_id: Uuid::new_v4(),
    target_file_path: None,
    source_file_paths: vec!["src/lib.rs".to_string()],
    include_failure_context: false,
    api_key: api_key,
}).await.unwrap();
```

### Health Check

The `/api/v1/llm-step/health` endpoint returns the current system status.

```bash
curl http://localhost:8080/api/v1/llm-step/health
# Response: {"status":"ok","node_count":0,"default_provider":"anthropic","default_model":"claude-sonnet-4-20250514"}
```

## Graceful Shutdown

1. Allow in-flight LLM generation requests to complete (up to configured timeout)
2. Flush any buffered events to the event bus
3. Save in-progress node state to repository (if persistence is configured)
4. Close HTTP client connections

## Common Failure Modes

### LLM Provider Unavailable

**Symptoms:**
- `LlmStepError::ProviderError` with 5xx or connection error
- Generation requests timeout after timeout_secs

**Recovery:**
1. The retry mechanism will attempt up to `max_retries` with exponential backoff
2. If all retries exhausted, the node transitions to `Failed` state
3. The execution engine may fall back to an alternative node or abort

**Prevention:**
- Configure appropriate timeouts (default 120s)
- Use `max_retries: 3` for transient failures
- Monitor provider health via `/health` endpoint

### Token Budget Exceeded

**Symptoms:**
- `LlmStepError::TokenBudgetExceeded` with used/max token counts

**Recovery:**
1. Retry with a smaller prompt (reduce source context size)
2. Use a model with larger context window
3. Increase token budget allocation

**Prevention:**
- Set `max_tokens` appropriately for the model
- Limit source file count with `max_source_files`
- Truncate context when exceeding `max_context_size`

### Source File Not Found

**Symptoms:**
- `LlmStepError::ContextBuildFailed` with filesystem error

**Recovery:**
1. Verify the repo_root is correctly configured
2. Check that source file paths are relative to repo_root
3. Ensure the files exist and are readable

### Invalid Node Configuration

**Symptoms:**
- `LlmStepError::InvalidConfiguration` during `create_node` or `execute_step`

**Recovery:**
1. Run `validate_node_config` to get detailed error messages
2. Fix the reported fields (model, provider, prompt template, etc.)
3. Re-create the node with valid configuration

## Configuration Reference

| Parameter | Default | Description |
|-----------|---------|-------------|
| `default_provider.provider_name` | `"anthropic"` | LLM provider name |
| `default_provider.default_model` | `"claude-sonnet-4-20250514"` | Default model identifier |
| `default_provider.api_url` | `"https://api.anthropic.com/v1/messages"` | Provider API endpoint |
| `default_provider.max_tokens` | `4096` | Maximum response tokens |
| `default_provider.temperature` | `0.7` | Generation temperature |
| `max_retries` | `3` | Maximum retry attempts |
| `default_timeout_secs` | `120` | Request timeout in seconds |
| `validate_before_execution` | `true` | Whether to validate config before LLM calls |
| `max_source_files` | `20` | Maximum source files in context |
| `max_context_size` | `100000` | Maximum context size in characters |
| `include_symbols` | `true` | Whether to include symbol definitions in context |

## Observability

### Key Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `llm_step.generations.total` | Counter | Total LLM generation calls |
| `llm_step.generations.success` | Counter | Successful generations |
| `llm_step.generations.failed` | Counter | Failed generations |
| `llm_step.generations.duration_ms` | Histogram | Generation latency |
| `llm_step.tokens.total` | Counter | Total tokens consumed |
| `llm_step.context.files` | Histogram | Source files per context |
| `llm_step.retries.total` | Counter | Total retry attempts |

### Logging

The module uses structured logging via the `tracing` crate with correlation IDs:

```rust
tracing::info!(
    node_id = %node.id,
    execution_id = %input.execution_id,
    model = %node.model_config.model,
    "llm_step.generation_started"
);
```

### Events

The module emits `LlmStepEvent` variants on the event bus:
- `ContextAssemblyStarted` / `ContextAssemblyCompleted` / `ContextAssemblyFailed`
- `GenerationStarted` / `GenerationCompleted` / `GenerationFailed`
- `GenerationRetried`
- `TokenBudgetExceeded`
- `OutputParsed` / `OutputParseFailed`
