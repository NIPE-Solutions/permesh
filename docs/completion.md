# Shell completion

Generate completions from the installed binary's command definitions:

```sh
permesh completion bash
permesh completion zsh
permesh completion fish
permesh completion powershell
permesh completion elvish
```

The command prints a script to stdout. It works outside a workspace, ignores
`--config`, and does not read configuration, resolve credentials, run plugins,
contact providers, or install anything. The script contains command names,
flags, help text and fixed choices. It does not suggest identities or configured
provider instances. Your shell may also offer ordinary local file completion.

`--json` is incompatible with this command and returns an input error (exit 2).
No headings, diagnostics or ANSI styling are mixed into a successful script,
even with `--color always` or verbose flags. A closed output pipe exits cleanly;
other output failures return exit 5. Scripts may change as the command tree or
completion generator evolves; they are not part of the JSON schema contract.

## Use in the current session

Bash:

```bash
source <(permesh completion bash)
```

Zsh, after initializing its completion system if your shell setup has not already
done so:

```zsh
autoload -Uz compinit
compinit
source <(permesh completion zsh)
```

Fish:

```fish
permesh completion fish | source
```

PowerShell:

```powershell
permesh completion powershell | Out-String | Invoke-Expression
```

These commands load locally generated shell code into the current session.
Permesh itself never changes shell profiles. To retain completions, save the
script in a user-owned location and load it through your shell's normal startup
configuration. For example, Fish supports
`~/.config/fish/completions/permesh.fish`; for PowerShell, add the command above
to your existing `$PROFILE`. Regenerate saved scripts after updating Permesh.
Remove the saved script or startup entry to uninstall completions.

Only the five listed shells are supported by the current generator. Elvish users
can save `permesh completion elvish` output and load it through their existing
Elvish configuration. No shell is inferred from the environment, and no dynamic
provider-backed completion is enabled.
