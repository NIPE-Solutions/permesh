# Access profiles

One YAML file per profile: the access a role in your organization is expected
to have. Entries use immutable provider IDs, never display names.

```yaml
profile: backend
description: Backend engineers
includes: [everyone]
grants:
  - {instance: github, group: "1234567"}                       # team membership
  - {instance: github, role: Read, resource: "R_kgDOExample"}   # direct role
```

Draft one from an existing team with
`permesh profile suggest --group github:1234567 --name backend > profiles/backend.yaml`,
then review it. See [docs/profiles.md](https://github.com/NIPE-Solutions/permesh/blob/main/docs/profiles.md).
