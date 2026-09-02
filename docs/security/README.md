# Security documentation

- [Threat model and security contract](threat-model.md)

The contract separates implementation-backed safeguards from planned limits. Changes to parsers,
serializers, model commands, render output, WASM exports, or dependencies must review this
document and add adversarial regression coverage where relevant.
