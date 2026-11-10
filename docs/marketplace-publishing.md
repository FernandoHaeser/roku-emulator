# Publishing the VS Code extension

The extension (`apps/vscode`) is published to the Visual Studio Marketplace
by `.github/workflows/vscode-release.yml`. The workflow signs in to Microsoft
Entra ID with GitHub's OIDC token (`vsce publish --azure-credential`), so no
personal access token (PAT) is created, stored or rotated.

This guide covers the one-time setup, every release, and troubleshooting.

## How it works

```
tag vscode-vX.Y.Z on master
  └─ job package   build, VSIX, GitHub pre-release
  └─ job publish   environment "marketplace" (waits for your approval)
       ├─ GitHub issues a short-lived OIDC token for
       │    repo:FernandoHaeser/roku-emulator:environment:marketplace
       ├─ azure/login exchanges it for an Entra ID token of the
       │    "roku-emulator-publish" app (federated credential)
       └─ vsce publish --azure-credential --pre-release
            (the app is a Contributor of the FernandoHaeser publisher)
```

Only jobs in the `marketplace` environment of this repository get a token the
app accepts, and each of them waits for a required reviewer.

## Cost

Nothing is paid. The Entra ID Free tier (included with any Microsoft account)
covers app registrations and federated credentials; no Azure subscription is
needed (`allow-no-subscriptions: true`). GitHub Actions and environments are
free for public repositories, and Marketplace publishing is free.

The Azure portal may offer a "free trial" or "create subscription" and ask for
a credit card. Skip it: open https://entra.microsoft.com (or search
"Microsoft Entra ID" in the portal) instead.

## One-time setup

### 1. Marketplace publisher

Done: publisher `FernandoHaeser` at https://marketplace.visualstudio.com/manage.
Its **ID** must equal `"publisher"` in `apps/vscode/package.json`
(case-sensitive; the display name does not matter).

### 2. Register the app in Microsoft Entra ID

1. Sign in to https://entra.microsoft.com with the Microsoft account that
   owns the publisher.
2. **App registrations > New registration**:
   - Name: `roku-emulator-publish`
   - Supported account types: **Single tenant**
   - Redirect URI: empty
3. **Register**, then copy from **Overview**:
   - **Application (client) ID**
   - **Directory (tenant) ID**

Do not create a client secret.

### 3. Federated credential

In the app: **Certificates & secrets > Federated credentials > Add
credential**:

| Field              | Value                                    |
| ------------------ | ---------------------------------------- |
| Scenario           | GitHub Actions deploying Azure resources |
| Organization       | `FernandoHaeser`                         |
| Repository         | `roku-emulator`                          |
| Entity type        | **Environment**                          |
| GitHub environment | `marketplace`                            |
| Name               | `github-marketplace`                     |

The subject must read
`repo:FernandoHaeser/roku-emulator:environment:marketplace`.

### 4. GitHub environment

https://github.com/FernandoHaeser/roku-emulator/settings/environments >
**New environment** `marketplace`:

- **Required reviewers**: yourself.
- **Deployment branches and tags**: keep "No restriction" until step 6 (the
  identity workflow runs from `develop`).
- **Environment variables** (variables, not secrets; the ids are not
  sensitive):
  - `AZURE_CLIENT_ID` = Application (client) ID
  - `AZURE_TENANT_ID` = Directory (tenant) ID

### 5. Add the app to the publisher

1. **Actions > Marketplace identity > Run workflow** (branch `develop`) and
   approve the `marketplace` deployment.
2. The log of "Azure DevOps identity of the app" prints
   `{"id": "...", "displayName": "..."}`. Copy the `id`.
3. https://marketplace.visualstudio.com/manage > publisher `FernandoHaeser` >
   **Members > Add**: paste the id, role **Contributor**.

### 6. Restrict the environment

`marketplace` > **Deployment branches and tags > Selected branches and tags**

> add the tag rule `vscode-v*`. From now on only release tags can deploy.

## Every release

The extension follows the Gitflow in [git-workflow.md](git-workflow.md):

1. On a branch into `develop`: bump `version` in `apps/vscode/package.json`
   and add the entry to `apps/vscode/CHANGELOG.md`.
2. Promote `develop > release > master` through pull requests.
3. Tag the merge commit on `master` (the tag must match the version):

   ```bash
   git switch master && git pull
   git tag -a vscode-v0.1.0 -m "vscode-v0.1.0"
   git push origin vscode-v0.1.0
   ```

4. **Actions > VS Code extension release**: `package` creates the GitHub
   pre-release with the VSIX; approve `publish`.
5. Check the listing at
   https://marketplace.visualstudio.com/items?itemName=FernandoHaeser.roku-emulator
   and install it in a clean profile:
   `code --profile release-check --install-extension FernandoHaeser.roku-emulator`.

Versions are published as Marketplace **pre-releases** (`--pre-release`)
while the project is in beta. Drop the flag in the workflow for the first
stable version.

## Troubleshooting

| Symptom                                                                     | Cause and fix                                                                                                                                                                                         |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Set AZURE_CLIENT_ID and AZURE_TENANT_ID in the marketplace environment`    | Variables missing, or created as repository variables/secrets instead of environment variables (step 4).                                                                                              |
| `AADSTS70021: No matching federated identity record found`                  | The federated credential subject differs: entity type must be Environment and the name exactly `marketplace` (step 3).                                                                                |
| `AADSTS700016: Application ... was not found in the directory`              | `AZURE_TENANT_ID` belongs to another directory than the app.                                                                                                                                          |
| Identity workflow fails on `app.vssps.visualstudio.com` (`TF400813` or 401) | The app has no Azure DevOps profile yet. Create a free organization at https://dev.azure.com, add the app under **Organization settings > Users** (Basic or Stakeholder), and run the workflow again. |
| `vsce publish`: `401` / "not authorized" for the publisher                  | The app is not a member of the publisher, or has a Reader role (step 5).                                                                                                                              |
| `Tag vscode-vX does not match package.json version`                         | Bump the version before tagging, or tag the right version.                                                                                                                                            |
| `publish` waits forever                                                     | It is waiting for your approval in the run page.                                                                                                                                                      |
| Deployment rejected by protection rules                                     | The run did not come from a `vscode-v*` tag (step 6).                                                                                                                                                 |

## Revoking access

Delete the federated credential (or the whole app registration) in Entra ID
and remove the member from the publisher. Nothing else holds credentials.
