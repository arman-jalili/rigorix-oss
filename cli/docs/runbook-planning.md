# Runbook: Planning Pipeline Module

> **Module:** `cli/src/planning/`
> **Version:** 0.1.0

## Overview

6-phase planning flow via `rigorix plan <intent>`: Budget check → Intent Classification (LLM) → Parameter Extraction → Graph Generation → Validation → Hash.

## Startup

PlanCommandService wraps engine's PlanningPipelineService. Called via dispatch_command().

## Failure Modes

| Failure | Recovery |
|---------|----------|
| No template match | Try more specific intent |
| Budget exceeded | Increase limits or simplify |
| Classification failed | Retry, check LLM API key |
