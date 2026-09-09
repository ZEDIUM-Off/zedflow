# Research: GitHub durable large evidence artifacts for a public repository

## Summary
For artifacts that must remain publicly downloadable beyond an Actions retention window, GitHub Releases are the relevant GitHub-hosted mechanism: a release permits up to 1,000 assets, each **under 2 GiB**, with **no stated total-release-size or bandwidth limit**. Releases are still administrator/writer-managed resources rather than permanent archival storage; immutable releases prevent post-publication tag/asset changes, but the release itself can be deleted.

## Findings
1. **Release capacity and delivery limits** — GitHub documents a maximum of **1,000 assets per release** and requires each asset to be **under 2 GiB**. It explicitly states **no limit on total release size or bandwidth usage**. This is the applicable official GitHub.com Release constraint; split evidence larger than 2 GiB per object. [About releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases#storage-and-bandwidth-quotas)
2. **Ordinary releases are deletable/mutable** — GitHub documents that users with write permission can manage releases, and its REST API says users with push access can delete a release. The releases API also supports creating, modifying, and deleting releases and release assets. Therefore, a normal release asset is durable only until an authorized actor removes or changes it—not immutable archival retention. [About releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases) · [Delete a release API](https://docs.github.com/en/rest/releases/releases#delete-a-release) · [Release asset API](https://docs.github.com/en/rest/releases/assets)
3. **Immutable releases protect published assets, with an important deletion boundary** — Once published, an immutable release locks its tag to a commit and prevents release assets from being modified or deleted. GitHub recommends draft → attach all assets → publish. However, its documentation explicitly says that **if the immutable release is deleted**, its tag may be deleted (though the tag name cannot be reused). Thus immutability is strong protection against per-asset/tag alteration while the release exists, not an undeletable preservation guarantee. [Immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases)
4. **Actions artifacts are not durable evidence storage for a public repository** — They default to automatic deletion after **90 days**; for public repositories, retention can only be configured from **1 to 90 days**. GitHub also permits deletion before expiry, says deletion is irreversible, and deletes all artifacts when their workflow run is deleted. Custom retention applies only to new artifacts. These facts make Actions artifacts suitable for workflow exchange/short-term build output, not long-lived public evidence. [Retention policy](https://docs.github.com/en/organizations/managing-organization-settings/configuring-the-retention-period-for-github-actions-artifacts-and-logs-in-your-organization) · [Removing workflow artifacts](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/remove-workflow-artifacts)

## Sources
- Kept: [About releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases) — primary GitHub.com statement of Release asset-count, per-file, aggregate-size, and bandwidth limits.
- Kept: [Immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases) — primary statement of immutable asset/tag behavior and the release-deletion boundary.
- Kept: [Delete a release API](https://docs.github.com/en/rest/releases/releases#delete-a-release) — primary authorization and endpoint evidence that releases remain deletable.
- Kept: [Release asset API](https://docs.github.com/en/rest/releases/assets) — primary evidence that non-immutable release assets are manageable/deletable.
- Kept: [Retention policy](https://docs.github.com/en/organizations/managing-organization-settings/configuring-the-retention-period-for-github-actions-artifacts-and-logs-in-your-organization) — current public-repository Actions retention range.
- Kept: [Removing workflow artifacts](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/remove-workflow-artifacts) — primary deletion, irreversibility, and deleted-workflow-run behavior.
- Dropped: Git LFS billing — different storage product; not needed to answer Release-asset durability.
- Dropped: Repository limits — Git-object/push limits, not Release asset limits.

## Gaps
GitHub’s cited Release documentation states no total-size or bandwidth limit but does not promise a retention duration, backup policy, SLA, or legal-record preservation. Use immutable Releases for GitHub-hosted integrity and long-lived availability, but keep an independently controlled archival copy if evidence must survive repository/release deletion or account loss.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Concrete findings are recorded in /home/zedium/workspaces/zedflow/research.md with official GitHub Docs URLs; no severity applies to this research-only task."
    }
  ],
  "changedFiles": [
    "/home/zedium/workspaces/zedflow/research.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "Official GitHub Docs web research and source verification",
      "result": "passed",
      "summary": "Verified current official documentation for Releases, immutable releases, Actions retention, and artifact deletion."
    }
  ],
  "validationOutput": [
    "All factual claims cite docs.github.com URLs.",
    "No repository code or configuration was modified."
  ],
  "residualRisks": [
    "GitHub documents no Release retention/SLA or backup guarantee; an authorized actor can delete a release, including an immutable release.",
    "The cited no-total-size/no-bandwidth statement is for Releases; it does not apply to Git LFS or Actions artifacts."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added the requested research brief at research.md only.",
  "reviewFindings": [
    "no blockers: findings distinguish immutable asset protection from deletion of the containing release.",
    "no blockers: Actions artifact retention is correctly limited to 1–90 days for public repositories."
  ],
  "manualNotes": "Research-only request; no source-code changes were made."
}
```