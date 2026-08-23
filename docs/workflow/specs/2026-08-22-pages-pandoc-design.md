# Pages Pandoc provisioning design

Issue: [#542](https://github.com/randomparity/bzr/issues/542)

Decision: [ADR 0020](../../adr/0020-pages-build-runs-on-pull-requests.md)

## Goal

Make the GitHub Pages build self-provisioning and exercise the production site
build before merge while preserving deployment only for trusted non-PR events.

## Architecture and flow

`.github/workflows/site.yml` remains the single owner of site build and
deployment. Pull requests targeting `main` use the same relevant path set as
pushes to `main`: `README.md`, `CHANGELOG.md`, `site/**`, `docs/assets/**`, and
`.github/workflows/site.yml`. A matching pull request, a matching push, or a
manual dispatch enters the same `build` job. That job checks out the repository,
installs Pandoc through `apt-get`, generates `_site/index.html` and
`_site/changelog.html`, verifies both are non-empty, and uploads `_site` as the
Pages artifact.

The `deploy` job depends on `build`. It is skipped when
`github.event_name == 'pull_request'`; pushes to `main` and manual dispatches
continue through the existing protected `github-pages` environment and
`actions/deploy-pages` step.

Workflow concurrency retains `pages` for pushes and manual dispatches and uses
`pages-pr-<number>` for pull requests. GitHub replaces an existing pending run
when another run enters the same concurrency group, even when
`cancel-in-progress` is false. Separate groups therefore keep PR activity from
replacing a pending production run.

## Failure behavior

The job fails at the first unavailable prerequisite, failed Pandoc conversion,
or empty expected output. Artifact upload and deployment therefore cannot run
after an incomplete build. The commands use the runner shell's existing
fail-fast behavior; no fallback artifact is produced.

## Verification

- The observed red case is Actions run 32606444127, where the production build
  exited 127 at the first Pandoc invocation.
- `actionlint .github/workflows/site.yml` validates workflow syntax.
- `zizmor .github/workflows/site.yml` validates the changed workflow security
  surface.
- A local Pandoc smoke build verifies both expected HTML files are non-empty.
- The pull request's Site workflow is the recurrence guard: it executes the
  exact build job on GitHub's `ubuntu-latest` runner before merge and must show
  deployment as skipped.
- After merge, the push-triggered Site workflow must build and deploy, and the
  public URL must return a 2xx response before issue #542 closes. A failed
  deployment or continuing non-2xx response keeps the issue open for
  investigation. This is post-merge evidence and is not claimed by pre-merge
  CI.

## Threat model

### Boundary inventory

- Existing files: repository-controlled Markdown and templates enter Pandoc in
  the GitHub-hosted build job.
- Added actor boundary: pull-request authors, including fork authors, can cause
  their proposed Markdown, template, and asset changes to enter Pandoc, trigger
  an Ubuntu package download, and produce a Pages artifact.
- Existing boundary retained: the deploy job writes to GitHub Pages using an
  OIDC token and the `pages: write` permission.

### Actor model

Pull-request authors control changes in the checked-out repository. GitHub and
Ubuntu's signed package repositories are trusted to provide the runner and
Pandoc package. Only non-PR events are trusted to reach deployment.

### Controls

- Workflow permissions remain `contents: read` at top level.
- The deploy job alone retains `pages: write` and `id-token: write`.
- The deploy job's event guard excludes pull requests before the environment or
  deployment action runs.
- Event-specific concurrency groups prevent pull requests from replacing a
  pending production deployment run.
- Pandoc receives repository files as files, not interpolated shell values.
- Output existence checks prevent upload of a silently incomplete site.

### Out of scope

This change does not execute repository scripts for pull requests and does not
add preview deployments, custom domains, content sanitization, or changes to
Pages environment protection.

## Scope boundaries

No CLI behavior, site content, visual styling, Pages URL, unrelated workflow,
or repository dependency manifest changes are included.
