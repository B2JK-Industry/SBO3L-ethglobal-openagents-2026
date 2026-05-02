{{/*
=========================================================================
EXPERIMENTAL — chart skeleton.
This chart deploys the SBO3L daemon as a single Deployment + Service.
CRDs (SBO3LPolicy, SBO3LCluster) are intentionally NOT shipped here:
they require a controller to act on them, and a controller without an
operator pod is inert. Multi-replica with shared storage / Raft is
deferred to a separate operator-chart project.
=========================================================================
*/}}

{{/*
Expand the name of the chart.
*/}}
{{- define "sbo3l.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited
to this (by the DNS naming spec).
*/}}
{{- define "sbo3l.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "sbo3l.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels — applied to every rendered resource.
*/}}
{{- define "sbo3l.labels" -}}
helm.sh/chart: {{ include "sbo3l.chart" . }}
{{ include "sbo3l.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: sbo3l
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Selector labels — these are used in Service / Deployment selectors and
must be stable across upgrades. Do NOT add release-specific labels here.
*/}}
{{- define "sbo3l.selectorLabels" -}}
app.kubernetes.io/name: {{ include "sbo3l.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Common annotations.
*/}}
{{- define "sbo3l.annotations" -}}
{{- with .Values.commonAnnotations }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Service account name to use.
*/}}
{{- define "sbo3l.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "sbo3l.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Image reference (repository:tag).
*/}}
{{- define "sbo3l.image" -}}
{{- $tag := .Values.image.tag | default .Chart.AppVersion -}}
{{- printf "%s:%s" .Values.image.repository $tag -}}
{{- end }}

{{/*
Secret name resolution: if `existingSecret` is set, reference that;
otherwise, if we render our own Secret, reference its name; otherwise
emit an empty string (which downstream conditionals must guard).
*/}}
{{- define "sbo3l.secretName" -}}
{{- if .Values.existingSecret -}}
{{ .Values.existingSecret }}
{{- else if .Values.secret.create -}}
{{ include "sbo3l.fullname" . }}
{{- end -}}
{{- end }}
