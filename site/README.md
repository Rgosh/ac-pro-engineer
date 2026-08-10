# The project's site

One page, `index.html`, self-contained: inline CSS, no fonts, no scripts, no
third-party anything. The palette is the application's own default theme from
`core/src/config.rs`, so the site and the program cannot drift apart in look
without somebody noticing.

Deployed by `.github/workflows/pages.yml` on any push to `main` that touches
`site/`, `screenshots/` or the workflow itself.

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

Add a file named `CNAME` in the **repository root** containing just the
hostname:

```
engineer.pro
```

The workflow copies it into the deploy. **Add the DNS record first.** A `CNAME`
file naming a hostname that does not resolve yet takes the site off the
`github.io` address without putting it anywhere else.

Afterwards, in `index.html`, point `rel="canonical"` and `og:url` at the new
address. Leaving them on the old one tells search engines the new site is a copy
of the old, which is the opposite of what a move is for.

### Subdomain or path

One repository serves **one** custom domain, so `ac.engineer.pro` and
`iracing.engineer.pro` would need a repository each.

Paths on one domain — `engineer.pro/ac` — keep everything in one place, and
search engines treat a subdomain as substantially its own site, so splitting
across them divides whatever authority the domain earns instead of pooling it.
Prefer paths unless the two things are genuinely separate projects.
