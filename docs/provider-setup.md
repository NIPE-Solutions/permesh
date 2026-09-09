# Set up a provider

## Official GitHub

```bash
permesh provider add github
```

In an existing workspace, this command downloads the newest release in the public
catalog compatible with the current host and declarative setup. `--version VERSION`
selects that exact stable release; `--id ID` replaces the default `github-main`.
A newer locally installed release does not override this selection.

The first prompt shows the source, version, reported capabilities and native-code
risk. Answer `yes` to trust the verified executable and run its setup description.
Checksums match the public catalog; publisher signatures are not verified. Use `-v`
for the executable path and digest. The second prompt reviews the resulting settings
and named credential references before approving workspace execution and credential
delivery. Neither step resolves credentials. Run `permesh auth login github-main`
separately for keychain references; supply environment references outside Permesh.

For automation, prepare the strict answer file described below using the provider's
setup field names and credential references, then explicitly authorize both steps:

```bash
permesh provider add github --id github-main --version VERSION \
  --answers github-answers.yaml --accept-risk --json
```

Noninteractive input, including `--json`, requires both `--answers` and
`--accept-risk`. JSON contains only the result envelope, with no prompt decoration.
Legacy organization/token flags and manual external settings are rejected: use the
form or answer file. Other providers retain their existing add/setup workflows.

Declining binary trust leaves only downloaded package files and executes no
provider. A setup failure retains the package and any completed local trust;
correct the answers and retry the returned exact-version add command. If an instance
was created but workspace approval was declined or failed, use the returned external
review/approve commands. Add never overwrites an existing instance. Detected workspace
edits during setup or approval prevent stale writes or approval.

Configuration schema 1 pins one current-host executable digest. A shared configuration
does not automatically select another platform's artifact; cross-platform release
pins require a separate configuration design. Install/update still only download
packages and never silently change instance pins or approvals.

## Registered and third-party providers

`permesh provider setup` asks a locally registered native provider for a declarative
setup description, then uses CLI-owned prompts or an answer file to add a workspace
instance. The provider must support draft-3 description exchanges. Built-in
providers continue to use `provider add`.

First [inspect and trust the native binary](external-providers.md). Setup executes
that verified registered copy as your user. Existing binary trust authorizes this
explicit execution; setup has no additional `--accept-risk` flag. Native code is
not sandboxed and can independently access files or the network.

## Describe without a workspace

```bash
permesh provider setup example --id example-main --describe --json
```

The validated schema appears under `result.spec`. This command does not load or
write a workspace. It cannot be combined with `--answers` or `--authoritative`.
The instance ID defaults to `REGISTERED_ID-main` when `--id` is omitted.

Description is the only operation sent to the provider during setup. The child
receives a handshake containing the instance ID, followed by `describe`. It
receives no answers, settings, credentials, answer-file name or workspace path.
The host clears the inherited environment and uses the same bounded streams,
deadlines and cancellation cleanup as other explicit external operations.

## Add an instance

In an existing workspace, use an interactive terminal:

```bash
permesh provider setup example --id example-main
```

The CLI asks active questions in declaration order. Enter text, choice values and
credential references as strings; enter integers and booleans as their typed
values. Lists and structured JSON fields use JSON input. Blank input accepts a
default when present, or skips an optional field. Required fields without a
default must be answered. Cancellation or end of input leaves the workspace
unchanged.

All configuration answers must be nonsecret. Credential questions accept only
`env://NAME` or a valid `keychain://INSTANCE/SLOT` reference. Setup neither
resolves those references nor stores credentials. Provider-authored labels cannot
establish that an arbitrary text field is safe for secrets: never paste a token,
password or private key into a settings field.

For automation, provide a strict YAML answer file:

```yaml
version: 1
answers:
  auth_method: token
  token: env://EXAMPLE_TOKEN
  tenant: example-tenant
```

```bash
permesh provider setup example --id example-main --answers answers.yaml --json
```

Noninteractive input and `--json` require `--answers` for a workspace mutation;
missing answers are rejected before launching the provider. Files are limited
to 64 KiB and must contain exactly `version: 1` and an `answers` mapping. JSON is
also accepted. Duplicate or unknown keys, extra documents and executable or
included content are not supported. Field names and values must match the
provider's validated description. Unknown or inactive answers are errors.
Optional `null` explicitly omits a field; required `null` is invalid.

The CLI checks answer-file syntax and bounds before launch. Field types and
credential-reference semantics are checked after description, when the schema
is available, and before any workspace write. Answers are never sent to the
provider. See the [synthetic examples](../examples/setup/README.md).

Setup refuses to replace an existing instance. It pins the registered ID and
SHA-256 in the new external instance, checks the registration again before
writing, and refuses a workspace changed during setup. Invalid schemas or
answers leave the file unchanged. `--authoritative` explicitly adds identity
source authority and requires the registered `identities` capability.

## Review before queries

Setup does not approve workspace execution. Review the resulting configuration
and use the returned fingerprint:

```bash
permesh provider external review example-main --json
permesh provider external approve example-main --fingerprint REVIEWED_FINGERPRINT --accept-risk
permesh doctor
```

For keychain references, store a value separately with
`permesh auth login example-main --slot token` (using the selected slot). Supply
environment values outside Permesh for `env://` references. Approval binds the selected instance's configuration and relevant identity
context. Adding an unrelated instance does not invalidate existing approvals.
See [workspace approval](external-providers.md) and [credentials](secrets.md).

## Provider-owned schema, CLI-owned questions

SDK `setup::SetupSpec` uses schema version 1. It contains `schema_version`, `title`,
`description` and ordered `steps`. A step contains `id`, `title`, `description`,
optional `when` and nonempty `fields`. A field contains `key`, `label`, `help`,
`required`, `input`, and optional `default` and `when`. All other fields are
required; unknown fields are rejected.

| Input `type` | Additional properties | Answer |
| --- | --- | --- |
| `text` | `min_length`, `max_length` | String |
| `integer` | `minimum`, `maximum` | Signed 64-bit integer |
| `boolean` | None | Boolean |
| `choice` | `options`: objects with `value`, `label` | One listed string value |
| `string_list` | `min_items`, `max_items` | Array of strings |
| `json` | None | Bounded JSON-compatible value |
| `credential` | None | Credential reference string |

`when: {field: auth_method, equals: token}` activates a step or field by comparing
an earlier scalar answer or default. Conditions can refer only to earlier text,
choice, integer or boolean fields. They cannot inspect credentials, lists or JSON,
or refer to themselves or later fields. Defaults must validate against the input;
null and credential defaults are rejected. There is no interpolation, executable
expression, regex, remote schema reference or provider-driven terminal UI.

Step IDs and globally unique field keys start with an ASCII letter and contain
only ASCII letters, digits, `_` or `-`, up to 64 characters. Specs are at most
64 KiB, with 1–32 steps, at most 128 fields and at most 16 credential fields.
Titles and labels are at most 128 characters; descriptions and help at most
1,024. Display text rejects control and bidi characters. Text is at most 4,096
characters. Lists have at most 128 strings, each at most 4,096 characters. Choice
lists contain 1–128 unique values. Answers and normalized configuration are
bounded to 64 KiB of JSON and 16 levels of nesting.

`validate` checks the description. `questions` validates supplied answers and
returns active fields in declaration order, allowing unanswered required fields
for prompting. `resolve` applies defaults, enforces required fields and separates
`configuration` from named `credentials`. The SDK bounds credential strings; the
CLI applies secret-reference and instance/slot rules.

Providers return a complete description in the [draft-3 exchange](provider-protocol.md#setup-description-draft-3).
Queries and health use the explicitly selected discovery contract. Official guided
add carries the verified package selection into workspace configuration. For
separately trusted negotiated-v1 binaries, pass `--discovery-protocol negotiated-v1`
to setup; the advanced workflow defaults to legacy for existing binaries. [Package installation and explicit updates](provider-packages.md) store untrusted binaries; use explicit binary trust before setup. Automatic updates and dynamic provider-driven setup steps remain deferred.
