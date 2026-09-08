# Declarative setup examples

These synthetic files illustrate [setup schema 1](../../docs/provider-setup.md).
They contain no credential values and do not install or execute a provider.

- `spec.json` includes all seven field types and conditional token/service branches.
- `answers.yaml` selects token authentication with an environment reference. Port
  and enabled use defaults; the service fields remain inactive.
- `service-answers.yaml` selects service credentials with a keychain reference
  bound to instance `example-main` and slot `client_secret`. Optional metadata
  is explicitly omitted with null.

A native provider implementing this spec returns it in the draft-3 setup terminal
frame. Merely saving a spec file does not register a provider or make the CLI use
it. After independently reviewing and trusting a compatible native provider as
`example`, run in an existing workspace:

```bash
permesh provider setup example --id example-main --describe --json
permesh provider setup example --id example-main --answers examples/setup/answers.yaml
```

Adjust the answer-file path to its actual location. Use one answer file per new
instance; setup refuses to replace an existing ID. For the service example, keep
`--id example-main` or update the keychain reference to match the chosen instance.

Setup writes configuration and references only. Review and explicitly approve
the workspace before queries. Supply the environment credential yourself or use
`permesh auth login example-main --slot client_secret` for the service example.
No example value is a working credential or real tenant identifier.
