# SBO3L CI / CD plugins

Capsule-verification plugins for non-GitHub CI/CD systems. Mirrors the GitHub Action shipped at `actions/sbo3l-verify/` (PR #286).

| Platform | Path | What it ships |
|---|---|---|
| GitHub Actions | `actions/sbo3l-verify/` | Composite action; PR comments + step summary |
| GitLab CI | `ci-plugins/gitlab/` | Drop-in `.gitlab-ci.yml` template + included job |
| CircleCI | `ci-plugins/circleci/orb/sbo3l/` | Orb source — publishable as `sbo3l/sbo3l@1.2.0` |
| Jenkins | `ci-plugins/jenkins/` | Pipeline shared-library Groovy script |

All four plugins:
- Verify a SBO3L Passport capsule against the same 6-check inline verifier (`is_object`, `type_recognised`, `decision_set`, `audit_event_id_present`, `request_hash_present`, `policy_hash_present`)
- Accept v2 capsule shape AND legacy receipt envelope
- Self-contained: no install at runtime
- Surface `decision`, `audit_event_id`, `checks-passed` to downstream steps

The verifier logic is the same `~110 LoC Node script` shipped by the GitHub Action — each plugin wraps it with platform-native plumbing (artifacts, contexts, environments).

See the per-plugin `DEPLOY.md` for the marketplace-publish steps Daniel needs to run.
