# Git workflow

Gitflow with three long-lived branches. All work reaches them through pull
requests; CI (`.github/workflows/ci.yml`) runs on every PR and push.

| Branch    | Role                            | Receives PRs from                    | Protected |
| --------- | ------------------------------- | ------------------------------------ | --------- |
| `develop` | Integration, default branch     | `feature/*`, `bugfix/*`, back-merges | yes       |
| `release` | Pre-production (beta/RC)        | `develop`, `bugfix/*`, `hotfix/*`    | yes       |
| `master`  | Production; every commit tagged | `release`, `hotfix/*`                | yes       |

`.github/workflows/branch-policy.yml` rejects PRs into `release`/`master`
from any other source branch.

```
feature/x ──▶ develop ──▶ release ──▶ master ──(tag vX.Y.Z)
                 ▲           │           │
                 └── back-merge ◀────────┘
hotfix/y ─────────────────────────────▶ master (+ back-merge to release, develop)
```

## Day to day

```bash
git switch develop && git pull
git switch -c feature/roku-url-transfer
# ... commits ...
git push -u origin feature/roku-url-transfer
gh pr create --base develop
```

Branch names: `feature/<topic>`, `bugfix/<topic>` (fix for something not yet
in production), `hotfix/<topic>` (urgent fix for production).

Commit messages follow Conventional Commits (`feat:`, `fix:`, `docs:`,
`refactor:`, `test:`, `chore:`).

## Releasing

1. Open a PR `develop -> release`. Move the `[Unreleased]` CHANGELOG section to
   the new version (e.g. `[0.2.0-beta]`) in that PR or right after it, on
   `release`.
2. Validate on `release` (pre-prod): `make check`, `make e2e`, and re-record
   probes on the device when the Roku OS changed (`make record PROBE=...`).
   Fixes go in `bugfix/*` PRs into `release`.
3. Open a PR `release -> master`. After merging, tag the merge commit on
   `master` and push the tag:

   ```bash
   git switch master && git pull
   git tag -a v0.2.0-beta -m "v0.2.0-beta"
   git push origin v0.2.0-beta
   ```

4. Back-merge `master` into `develop` (PR `master -> develop`) so release fixes
   are not lost.

## Hotfixes

1. `git switch -c hotfix/<topic> master`.
2. PR into `master`; tag a patch version after merging.
3. Back-merge `master` into `release` and `develop`.

## Merge strategy

- `feature/*`, `bugfix/*` into `develop`: squash merge (one commit per change).
- `develop -> release` and `release -> master`: merge commit (keeps the
  promotion visible and lets back-merges fast-forward cleanly).
