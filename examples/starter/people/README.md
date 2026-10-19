# People

One YAML file per person, naming their canonical identity (usually the
authoritative directory email) and the profiles they hold:

```yaml
identity: alice@example.com
profiles: [backend]
```

People can also receive profiles from directory group membership through the
`profiles.directory_groups` section of `permesh.yaml`. Run `permesh profile check`
to compare these declarations with observed access.
