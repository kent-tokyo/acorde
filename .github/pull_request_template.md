## Review checklist

- [ ] Tests and fixtures cover the changed behavior and malformed-input path where applicable.
- [ ] Core/io/layout remain synchronous and filesystem-free; renderer changes remain DOM-free.
- [ ] Parser, serializer, WASM, CLI, dependency, or archive changes were reviewed against the
      [security contract](../docs/security/threat-model.md).
- [ ] Resource limits, typed errors, determinism, and compatibility/loss diagnostics remain
      explicit.
- [ ] External or host-owned capabilities are labeled as such and are not claimed as local
      implementation evidence.
