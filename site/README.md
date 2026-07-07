# utxray-site

Official website for utxray — <https://utxray.paopao.studio>.

Astro static site, bubble-light design system. One human landing page plus
plain-text agent endpoints (`/llms.txt`, `/skill.md`, `/manifest.md`,
`/install.sh`). The endpoints are **copied from the repo at build time**
(`npm run sync`, wired into `predev`/`prebuild`), so the site can never drift
from the tool.

## Develop

```bash
npm install
npm run dev        # http://localhost:4321 (runs sync first)
npm run build      # static build into dist/
npm run preview    # serve dist/ as production would
```

## Deploy

Hosted as an assets-only Cloudflare Worker (`wrangler.jsonc`), deployed from
GitHub Actions (`.github/workflows/site.yml`):

| Trigger | What happens |
| --- | --- |
| any PR touching `site/**` or the synced sources* | build check (forks included — no secrets involved) |
| same-repo PR | + preview deploy (`wrangler versions upload`), URL commented on the PR |
| push to `main` | production deploy (`wrangler deploy`, GitHub environment `production`) |

\* the synced sources are `skills/aiken-contract-dev/SKILL.md`,
`docs/command-manifest.md` and `install.sh` — changes to them redeploy the
site so the published agent endpoints stay current.

`404.astro` + `not_found_handling: "404-page"` serve the 404 page.

### One-time wiring

1. **Cloudflare** — create a custom API token: *Account → Workers Scripts →
   Edit* (nothing else). Copy your Account ID (Workers & Pages overview).
2. **GitHub** — repo *Settings → Secrets and variables → Actions*: add
   `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`. Optionally create the
   `production` environment and add protection rules (required reviewers).
3. **First deploy** — merge any `site/**` change to `main`; this creates the
   `utxray-site` Worker. Preview uploads on PRs work from then on.
4. **Domain** — Worker → *Settings → Domains & Routes* → add
   `utxray.paopao.studio` (the same value is set in `astro.config.mjs`
   `site:` so canonical URLs are correct).

Fork PRs get the build check but no preview (they can't read secrets — by
design). To preview a community PR, push its branch to this repo:
`git push origin pr-123:preview/pr-123`, then open a same-repo PR.
