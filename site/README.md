# The project's site

One page, `index.html`, self-contained: inline CSS, no fonts, no scripts, no
third-party anything. The palette is the application's own default theme from
`core/src/config.rs`, so the site and the program cannot drift apart in look
without somebody noticing.

## Two copies, on purpose

The same page is served from two addresses and neither redirects to the other:

* **proengineer.app** — a host of its own, uploaded by `site/deploy.sh`.
* **rgosh.github.io/ac-pro-engineer** — GitHub Pages, deployed by
  `.github/workflows/pages.yml` on any push to `main` that touches `site/`,
  `screenshots/` or the workflow itself.

Both declare `rel="canonical"` pointing at **proengineer.app**. That is what
stops two identical pages competing: search engines index the domain and treat
the github.io copy as the same document, while a person reaching either one
stays on the address they arrived at.

So a change is deployed twice — push for one, `./site/deploy.sh` for the other.

**`site/deploy.sh` is untracked.** It names a host alias and server paths, and
the layout of a machine is not something a public repository needs to publish,
so it is in `.gitignore`. That also means a fresh clone will not have it: keep a
copy somewhere outside the repository. It contains no credentials — the host is
an entry in `~/.ssh/config` and its key stays in `~/.ssh`, which is the only
place a private key belongs.

## Editing it

Open `index.html` in a browser. That is the whole toolchain.

The pictures are **not** copied into `site/`. The page reads them from
`screenshots/` — the same paths the README uses — and the workflow puts the two
together at deploy time. One copy, so refreshing the screenshots updates the
site as well. A build step fails if the page names a picture that is not there.

## Counts that have to stay true

The heading says "ALL 65 FEATURES", the stat strip says 65, each fold has a
count and each card in *What you get* has a badge. All four are checked against
the number of table rows before deploy. **Adding a feature row means updating
that fold's count, its card badge, the stat strip and the heading** — or the
check fails, which is the point: a landing page that overstates by six is worse
than one that says nothing.

## Putting it on your own domain

GitHub Pages serves a custom domain itself and issues a free certificate for it.
There is no server to run and nothing to pay for beyond the domain.

### With Cloudflare as the registrar

Cloudflare requires the domain to use its own nameservers, which it sets up when
you buy. Two things about that combination are worth knowing before you start,
because both fail in ways that look like something else.

**Use CNAME flattening at the apex.** Most registrars cannot put a `CNAME` on a
bare domain and you have to use four `A` records to GitHub's addresses — which
change from time to time, so they rot. Cloudflare flattens a `CNAME` at the
apex, so one record does it and there is nothing to keep up to date:

```
Type    Name    Target              Proxy
CNAME   @       rgosh.github.io     DNS only
```

**Leave the proxy off (grey cloud) at least until it works.** With the orange
cloud on and SSL/TLS mode set to *Flexible*, Cloudflare talks to GitHub over
plain HTTP while telling the browser the connection is secure; GitHub redirects
to HTTPS, and the two bounce until the browser gives up — a redirect loop that
looks nothing like a TLS setting. It also stops GitHub issuing its own
certificate, because the validation request never reaches it. Grey cloud first;
if you later want the proxy, set SSL/TLS to **Full (strict)** at the same time.

### Then, and only then

`CNAME` in the **repository root** holds the hostname — `proengineer.app` — and
the workflow copies it into the deploy. **The DNS record goes in first.** A
`CNAME` file naming a hostname that does not resolve yet takes the site off the
`github.io` address without putting it anywhere else.

Four absolute addresses live in `index.html`: `rel="canonical"`, `og:url`,
`og:image` and the `url` in the JSON-LD block. They move together with the
domain and nothing else in the file does — every other path is relative.

### Subdomain or path

One repository serves **one** custom domain. `proengineer.app` is this one, so
`ac.proengineer.app` and `acc.proengineer.app` would each need a repository of
their own, with their own `CNAME` file and their own deploy.

Paths on one domain — `proengineer.app/ac` — need none of that: the workflow
copies `site/` wholesale, so a game gets a folder under it and that is the whole
change. Search engines also treat a subdomain as substantially its own site, so
splitting across them divides whatever authority the domain earns rather than
pooling it on one.

Prefer paths while this is one program that supports several games, which is
what the architecture describes. Subdomains earn their keep when the things
behind them are genuinely separate products.
