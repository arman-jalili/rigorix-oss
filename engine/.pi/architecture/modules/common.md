# Common Module

Shared types, validation helpers, and utility functions used across all engine modules.

## Validation

The `validation` submodule provides composable validation functions:

- `ValidationResult` — success/failure with error collection
- `ValidationError` — typed validation error with field path
- Composable validators for common patterns (non-empty, max length, regex)
