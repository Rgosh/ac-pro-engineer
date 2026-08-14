# The project's site

Seven pages, dark, monospace, no fonts and no third-party anything. The palette
is the application's own default theme from `core/src/config.rs`, so the site
and the program cannot drift apart in look without somebody noticing.

## What is where

```
site/
  index.html          the landing — TRACKED, and the only tracked page
  assets/pe.css       the stylesheet every other page links
  pages/*.html        the body of each page, one file each
  build.py            assembles pages/ into the folders below
  inline-css.py       copies pe.css into the landing's <style> block
  deploy.sh           uploads the lot to proengineer.app
  ac/ features/ compare/ technical/ download/ faq/     generated
  sitemap.xml         generated
```

**Everything except `index.html` and the static assets is in `.gitignore`.**
That is deliberate: the deep pages are served from proengineer.app and are
deliberately not published to GitHub. The consequence is worth saying plainly —
**those files exist in this working copy and on the web server, and nowhere
else.** Git is not backing them up. Keep a copy outside the repository, the same
way `deploy.sh` has always had to be kept.

## Two copies, on purpose

The same landing is served from two addresses and neither redirects to the
other:

* **proengineer.app** — the whole site, uploaded by `site/deploy.sh`.
* **rgosh.github.io/ac-pro-engineer** — the landing alone, deployed by
  `.github/workflows/pages.yml` on any push to `main` that touches `site/`,
  `screenshots/` or the workflow itself.

The landing's own links are **absolute**, pointing at proengineer.app. That is
what makes the second copy work rather than break: it has none of the other
pages beside it, so every nav entry and every card on it opens the real site
instead of a 404. Both declare `rel="canonical"` at proengineer.app, so search
engines index the domain and treat the mirror as the same document.

The landing is also the one page whose stylesheet is **inlined** — it has to
stand up on Pages with nothing beside it. `inline-css.py` copies `pe.css` into
the block between the `/*PE_CSS*/` markers, so there are two copies of the
stylesheet and only one of them is written by hand.

## Building and deploying

```bash
python3 site/build.py
```

```bash
./site/deploy.sh
```

`build.py` is idempotent and `deploy.sh` runs it first, so a forgotten build
cannot ship. Deploying checks that every screenshot named by any page exists,
uploads, and then asks the live site for each page in turn — a deploy that
leaves a 404 behind exits non-zero.

## Adding a simulator

The point of the folder-per-game boundary in `core/src/games/` is that a second
simulator is an addition. The site is built the same way, and it takes four
steps:

1. Add an entry to `GAMES` in `build.py`. `state` decides the badge and where
   the tile links; use `"full"` only when the game genuinely works end to end.
2. Write `site/pages/<slug>.html` — the body only, no head and no nav. Copy
   `pages/ac.html` and work through it; it is the reference.
3. Add the page to `PAGES` in `build.py` for its nav entry, title, description
   and sitemap line.
4. Add the new folder to `.gitignore`, then build and deploy.

Nothing else knows the list. The landing's simulator picker, every nav strip,
the footer and the sitemap all come from those two lists.

## Fragment format

A fragment is three parts, in this order, cut apart by two markers:

```html
    <h1 class="h1">…</h1>          the hero, inside the terminal window
<!--SECTIONS-->
  <section id="…">…</section>      the body, at the page's full width
<!--LD-->
<script type="application/ld+json">…</script>
```

The structured data lives in the same file as the prose it describes, because
the failure that matters is not untidiness — it is JSON-LD still describing a
page that no longer says that.

## The pictures

Not copied into `site/`. Every page reads them from `screenshots/` — the same
paths the README uses — and both the deploy script and the Pages workflow put
the two together. One copy, so refreshing the screenshots updates the site as
well, and a build fails if a page names a picture that is not there.

## Counts that have to stay true

`/features/` says 115 in the stat strip and gives each fold its own count.
**Adding a feature row means updating that fold's count and the strip** — a
landing page that overstates by six is worse than one that says nothing.

## Favicons, and why there are so many

Google only uses an icon that is square and a **multiple of 48 px**, and it
reads the declarations on the site's home page rather than the web manifest.
The page used to declare 32 and 16 only, with 192 and 512 mentioned in
`site.webmanifest` — so there was nothing there it would take, and the search
result showed a globe while the browser tab showed the mark. 48, 96, 144 and 192
are now declared in the head; 32 and 16 stay for the tab, which has no such
rule; 512 is left to the manifest.

They carry no `?v=` query. That existed to get browsers past a cached 404 from
when this domain was on Pages with no `/favicon.ico`; the new filenames have no
cache to defeat, and a URL that keeps changing is a URL a search engine has to
keep re-learning.

Regenerating them from the 512 px master:

```bash
python3 -c "from PIL import Image; s=Image.open('site/icon-512.png').convert('RGBA'); [s.resize((n,n), Image.LANCZOS).save(f'site/icon-{n}.png', optimize=True) for n in (48,96,144)]"
```

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

`CNAME` in the **repository root** holds the hostname, and the workflow copies
it into the deploy. **The DNS record goes in first.** A `CNAME` file naming a
hostname that does not resolve yet takes the site off the `github.io` address
without putting it anywhere else.

The domain appears in `SITE` at the top of `build.py`, in the landing's
`rel="canonical"`, `og:url` and JSON-LD, and in the landing's own nav links.
Changing it means changing those; nothing else in the tree has an absolute
address in it.

### Subdomain or path

One repository serves **one** custom domain. `proengineer.app` is this one, so
`ac.proengineer.app` would need a repository of its own, with its own `CNAME`
and its own deploy.

Paths on one domain — `proengineer.app/ac` — need none of that, which is why
the game pages are laid out that way. Search engines also treat a subdomain as
substantially its own site, so splitting across them divides whatever authority
the domain earns rather than pooling it on one.
