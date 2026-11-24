# ThemeXpert fork of Nuphos

This is ThemeXpert's fork of [nuphos/nuphos](https://github.com/nuphos/nuphos)
(Apache-2.0). It adds what we need to run an always-on team of Bots, and
keeps taking upstream changes.

## Branches

| Branch | What it is | Rules |
| --- | --- | --- |
| `main` | Exact mirror of upstream `main` | Fast-forward only. Never commit here. |
| `themexpert` | Default branch: upstream + our changes | Feature branches merge in through PRs. |
| `tx/<topic>` | Our feature work | Branch from `themexpert`, PR into `themexpert`. |
| `upstream/<topic>` | A fix we want to give back | Branch from `main`, PR to `nuphos/nuphos`. |

## Pull upstream changes

```sh
scripts/themexpert/sync-upstream.sh
```

It fast-forwards `main` to upstream, merges `main` into `themexpert`, and
pushes both. On conflicts it stops and tells you what to run.

Remotes: `origin` = `themexpert/nuphos`, `upstream` = `nuphos/nuphos`
(push disabled).

## Keeping merges easy

- Prefer **new files** over editing upstream files: new routes, tools, skills
  and CLI subcommands in their own modules, registered with a one-line hook in
  the upstream file.
- Where an upstream file must change, keep the edit small and mark it with a
  `themexpert:` comment so conflicts are easy to spot.
- Generic improvements (bug fixes, features any Nuphos user would want) go
  upstream as a PR from an `upstream/<topic>` branch. The less we carry, the
  easier syncing is.
- Sync often (weekly at least). Small merges are easy; big ones are not.

## CI

GitHub keeps a fork's Actions off until someone enables them in the Actions tab.
Before enabling, note that upstream workflows include release jobs that need
upstream secrets, and jobs that target self-hosted `instabox-*` runners. Enable
only `ci.yml`-style checks, or add our own workflow under
`.github/workflows/themexpert-*.yml`.
