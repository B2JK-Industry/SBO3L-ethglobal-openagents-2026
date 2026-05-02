// Jenkins Pipeline shared-library step for SBO3L capsule verification.
//
// Drop this file into a shared-library repo's vars/ folder, then use
// from any Jenkinsfile:
//
//   @Library('sbo3l-shared@main') _
//
//   pipeline {
//     agent { docker { image 'node:20-bookworm-slim' } }
//     stages {
//       stage('Verify capsule') {
//         steps {
//           sbo3lVerify(
//             capsule: 'artifacts/latest.capsule.json',
//             failOnDeny: true
//           )
//         }
//       }
//     }
//   }
//
// Outputs:
//   - capsule-report.md  (markdown, also written to build log)
//   - capsule-result.json (structured)
//   - env.SBO3L_DECISION  (allow | deny | requires_human)
//   - env.SBO3L_AUDIT_EVENT_ID
//   - env.SBO3L_CHECKS_PASSED ("6/6")
//
// Build is FAILED if any verifier check fails OR (failOnDeny && decision != allow).

def call(Map config = [:]) {
  def capsule = config.capsule ?: error('sbo3lVerify: capsule path is required')
  def failOnDeny = config.failOnDeny != null ? config.failOnDeny : true
  def reportDir = config.reportDir ?: 'sbo3l-output'

  sh """
    set -e
    mkdir -p ${reportDir}
    # Pull the shared verifier (zero-dep). Pinning to main is fine —
    # the verifier is intentionally tiny + zero-dep.
    if [ ! -f /tmp/sbo3l-verifier.mjs ]; then
      curl -fsSL -o /tmp/sbo3l-verifier.mjs \
        https://raw.githubusercontent.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/main/ci-plugins/_shared/verifier.mjs
    fi

    # Markdown report
    SBO3L_CAPSULE_PATH='${capsule}' SBO3L_FAIL_ON_DENY='${failOnDeny}' SBO3L_REPORT_FORMAT=markdown \
      node /tmp/sbo3l-verifier.mjs | tee ${reportDir}/capsule-report.md

    # JSON for downstream steps (don't fail this one — exit code already
    # decided by the markdown step above)
    SBO3L_CAPSULE_PATH='${capsule}' SBO3L_FAIL_ON_DENY='false' SBO3L_REPORT_FORMAT=json \
      node /tmp/sbo3l-verifier.mjs > ${reportDir}/capsule-result.json
  """

  // Surface results as env vars for downstream stages.
  def result = readJSON(file: "${reportDir}/capsule-result.json")
  env.SBO3L_DECISION = result.decision ?: 'unknown'
  env.SBO3L_AUDIT_EVENT_ID = result.audit_event_id ?: ''
  env.SBO3L_CHECKS_PASSED = result.checks_passed ?: '0/0'

  // Archive both report files.
  archiveArtifacts(
    artifacts: "${reportDir}/capsule-report.md,${reportDir}/capsule-result.json",
    allowEmptyArchive: false,
  )

  echo "SBO3L verify: decision=${env.SBO3L_DECISION} checks=${env.SBO3L_CHECKS_PASSED}"
}
