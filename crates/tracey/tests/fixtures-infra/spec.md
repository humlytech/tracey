# Infrastructure Spec

r[infra.bucket.versioning]
Log buckets MUST have versioning enabled.

r[infra.region.pinned]
The deployment region MUST be pinned per environment.

r[deploy.image.minimal]
Runtime images MUST be built from a minimal base image.

r[deploy.image.dev-tools]
Development images MAY include extra tooling.

r[build.context.slim]
The build context MUST exclude dependency and build directories.

r[deploy.image.nonroot]
Containers MUST run as a non-root user.

r[deploy.image.healthcheck]
Long-running containers SHOULD declare a healthcheck.

This one is referenced only after a trailing `#` in the Dockerfile, where Docker
does not strip comments, so it MUST stay uncovered.
