# Jenkins — `sbo3lVerify` shared library step

## What this is
A Jenkins Pipeline shared-library step (`vars/sbo3lVerify.groovy`) that wraps the same shared verifier as the GitHub Action / GitLab template / CircleCI orb. Six verifier checks; archives `capsule-report.md` + `capsule-result.json`; surfaces `env.SBO3L_DECISION` / `env.SBO3L_AUDIT_EVENT_ID` / `env.SBO3L_CHECKS_PASSED` for downstream stages.

## Install (one-shot, by Daniel)

1. Create a new Git repo (or fork an existing shared-library repo) named e.g. `sbo3l-jenkins-shared`.
2. Copy the `vars/sbo3lVerify.groovy` file into the repo's root.
3. In **Manage Jenkins → System → Global Pipeline Libraries**, add a new library:
   - Name: `sbo3l-shared`
   - Default version: `main` (or pin to a tag for production)
   - Retrieval method: Modern SCM → Git → URL of the new repo

That's it — every Jenkinsfile in the controller can now call `@Library('sbo3l-shared@main') _` followed by `sbo3lVerify(capsule: '...')`.

## Use

```groovy
@Library('sbo3l-shared@main') _

pipeline {
  agent {
    docker {
      image 'node:20-bookworm-slim'
      args '-u root'
    }
  }
  stages {
    stage('Generate capsule') {
      steps {
        sh 'mkdir -p artifacts && ./generate.sh > artifacts/latest.capsule.json'
      }
    }
    stage('Verify capsule') {
      steps {
        sbo3lVerify(
          capsule: 'artifacts/latest.capsule.json',
          failOnDeny: true,
        )
      }
    }
    stage('Branch on decision') {
      when { expression { env.SBO3L_DECISION == 'allow' } }
      steps {
        echo "Audit event: ${env.SBO3L_AUDIT_EVENT_ID}"
        sh './deploy.sh'
      }
    }
  }
}
```

## Out of scope

- Jenkins **plugin** (.hpi) — the shared-library route hits 95% of the use case with zero plugin packaging / Marketplace approval cycle. A plugin counterpart is a future PR.
- GUI form for inputs — Jenkins Marketplace plugins are the right surface for that; deferred.
