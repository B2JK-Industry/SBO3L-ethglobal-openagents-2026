# CircleCI orb — `sbo3l/sbo3l`

## Local validation

```bash
circleci orb validate ci-plugins/circleci/orb/sbo3l/orb.yml
```

## First-time publish (one-shot, by Daniel)

```bash
# 1. Create the namespace (requires the org to own it on CircleCI)
circleci namespace create sbo3l B2JK-Industry github

# 2. Create the orb in the namespace
circleci orb create sbo3l/sbo3l

# 3. Publish a development version (bump as needed)
circleci orb publish ci-plugins/circleci/orb/sbo3l/orb.yml sbo3l/sbo3l@dev:0.0.1

# 4. After smoke testing, promote to production
circleci orb publish promote sbo3l/sbo3l@dev:0.0.1 patch
# → publishes sbo3l/sbo3l@1.2.0
```

## Subsequent publishes (automatable)

```bash
circleci orb publish ci-plugins/circleci/orb/sbo3l/orb.yml sbo3l/sbo3l@1.2.1
```

A future `.github/workflows/circleci-orb-publish.yml` could automate this — out of scope for v1.

## Use

```yaml
version: 2.1
orbs:
  sbo3l: sbo3l/sbo3l@1.2.0
jobs:
  my-job:
    docker:
      - image: cimg/base:stable
    steps:
      - checkout
      - run: ./generate-capsule.sh > artifacts/latest.capsule.json
      - sbo3l/verify:
          capsule: artifacts/latest.capsule.json
workflows:
  verify:
    jobs:
      - my-job
```

## Outputs

- `/tmp/sbo3l/capsule-report.md` — markdown table (job log + artifact)
- `/tmp/sbo3l/capsule-result.json` — structured JSON
- Stored as CircleCI artifacts under the `sbo3l` destination
