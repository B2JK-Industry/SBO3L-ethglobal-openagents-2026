# GitLab — `sbo3l-verify`

## Use as a remote include

```yaml
include:
  - remote: 'https://raw.githubusercontent.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/main/ci-plugins/gitlab/sbo3l-verify.gitlab-ci.yml'

sbo3l_verify:
  extends: .sbo3l_verify
  variables:
    SBO3L_CAPSULE_PATH: artifacts/latest.capsule.json
```

That's it — works from any GitLab project.

## Use locally vendored

If your org policy disallows remote includes, copy `sbo3l-verify.gitlab-ci.yml` into your own repo and bump the `before_script` to `cp` from local rather than `curl` from raw GitHub.

## Outputs

- `capsule-report.md` — markdown table (job log + artifact)
- `capsule-result.json` — structured `{ decision, audit_event_id, checks_passed, checks: [...] }`
- Job exit: 0 on allow + 6/6, 1 otherwise

Downstream jobs read via `dependencies: [sbo3l_verify]` + `artifacts.paths`.

## No publishing required

GitLab includes work via raw URL; no marketplace registration needed.
