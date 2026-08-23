# ADR 0020: Pages builds run on pull requests; deployment remains gated

## Status

Accepted for the implementing change of issue #542.

## Context

The first Pages workflow reached `main` without executing in its pull request.
Its build then failed because `ubuntu-latest` did not contain Pandoc, leaving no
artifact to deploy and the public URL returning 404. The build and deployment
currently share one workflow, so adding pull-request coverage must not grant a
pull request a deployment path.

## Decision

The Site workflow runs its existing build job for path-matched pull requests as
well as pushes to `main` and manual dispatches. The build explicitly installs
Pandoc from the Ubuntu package repository, generates both pages, and rejects
missing or empty output files before uploading the Pages artifact.

The deploy job retains its existing Pages permissions and environment, but an
event guard skips that job for pull requests. Pushes to `main` and manual
dispatches keep the deployment behavior they had before pull-request coverage
was added.

Pull-request and production runs use distinct concurrency groups. Production
runs retain the existing `pages` group, while each pull request uses a group
keyed by its number. This separation is required because GitHub replaces an
existing pending run when another run enters the same concurrency group, even
when `cancel-in-progress` is false.

## Consequences

- The same commands that produce the deployed artifact execute before merge.
- Pull requests download an Ubuntu package and upload a temporary Pages
  artifact, but cannot invoke `actions/deploy-pages`.
- Pull-request builds cannot replace a pending production deployment run.
- Package availability is owned by Ubuntu's repository for the runner image;
  an unavailable package fails visibly before site generation.
- The post-merge deployment remains the final proof of the public URL because a
  pull request cannot safely perform that deployment.

## Considered & rejected

- **Use a third-party Pandoc setup action.** judgment: one Ubuntu package does
  not justify another action and pinned supply-chain dependency.
- **Build inside a Pandoc container.** judgment: container lifecycle and image
  selection add more machinery than the two-command static-site build needs.
- **Only install Pandoc on pushes to `main`.** verified: Actions run
  32606444127 failed only after merge, demonstrating that post-merge-only
  execution does not protect the public site from a missing prerequisite.
- **Deploy from pull requests.** judgment: preview deployment was not requested,
  and widening the Pages write path is unnecessary to verify artifact creation.
