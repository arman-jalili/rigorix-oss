# Runbook: Template Generation Module

> **Module:** `cli/src/template_generation/`
> **Version:** 0.1.0
> **Last Updated:** 2026-06-16

## Overview

LLM-based template creation from natural language via `rigorix generate <intent>`.

## Startup

GenerateCommandService wraps the engine's TemplateGenerator. Called via `dispatch_command()` when user runs `rigorix generate`.

## Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| LLM error | GenerationFailed | Retry, check API key |
| Budget exceeded | BudgetExceeded | Increase budget or simplify intent |
| Invalid generated TOML | ValidationFailed | Retry with more specific intent |
