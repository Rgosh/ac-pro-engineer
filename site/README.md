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

GitHub Pages serves a custom domain itself, with a free certificate. There is no
server to run and nothing to pay for beyond the domain.

A **subdomain** is the easy case — one DNS record:

```
CNAME   ac   rgosh.github.io.
```

An **apex** domain (`example.com` with no prefix) needs four `A` records to
GitHub's addresses instead, or an `ALIAS`/`ANAME` if the registrar supports one.
GitHub's own documentation lists the current addresses; do not copy them from
here, because they have changed before.

Then add a `CNAME` file **in the repository root** containing just the hostname:

```
ac.engineer.pro
```

The workflow copies it into the deploy if it exists. Add the DNS record *first*:
a `CNAME` file pointing at a hostname that does not resolve yet takes the site
off the `github.io` address without putting it anywhere else.

One repository serves **one** custom domain. A second hostname means a second
repository.
