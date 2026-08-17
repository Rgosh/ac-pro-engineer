# Licensing

Pro Engineer is licensed under the **GNU Affero General Public License,
version 3** — `LICENSE` holds the text, `NOTICE` the two additional terms
section 7 permits. This file is the plain-language version of what that means,
because nobody should have to read a legal document to find out whether they
may use a telemetry app.

## The short version

| What you want to do | What you need |
|---|---|
| Use it, race with it, tell people about it | Nothing. It is free. |
| Read the source, learn from it, fork it | Nothing. |
| Change it for yourself and never hand it to anyone | Nothing. [Really nothing.](#private-use-is-not-restricted-at-all) |
| Build something on it and publish it **with source** under the AGPL | Nothing. Keep the notices, name the project. |
| Build something on it and keep **your** source closed | Written permission, in advance — [ask](#asking-for-a-closed-source-exception). |
| Sell a product with this code inside it | A commercial licence — [ask](#commercial-licensing). |

## Private use is not restricted at all

The AGPL puts conditions on **conveying** the software — handing a copy, or a
program built from it, to somebody else. It puts none on what you do with your
own copy on your own machine.

So: rebuild it, rip out half of it, wire it into your own closed tooling, run it
on your rig and your team's rigs, keep every line of it to yourself. No
permission, no notification, no obligation. This is the one part of the licence
that is genuinely absolute, and the fear of it is the most common
misunderstanding of the AGPL.

The line is reached when a copy leaves you, and section 13 draws it in one more
place than the plain GPL does: if people **use** your version over a network —
a hosted dashboard, a public relay, a service other drivers connect to — they
count as having received it, and they are owed its source. Your own machines
talking to each other are not other people.

## Using it in an open project

This is the case the licence is designed for and it needs no permission at all.
Publish your work under the AGPL v3, keep the copyright notices from `NOTICE`
intact, and credit Pro Engineer where your users can see it — the About
screen, the README, the credits, whichever your project has.

Two things the AGPL asks that the MIT licence did not:

- **The source has to be available to the people who run it.** Not just to
  whoever downloads a binary — if your work is reachable over a network,
  section 13 says its users get the source too. A telemetry relay, a web
  dashboard and a hosted engineer all count.
- **Your whole work goes under the AGPL**, not only the files you copied.
  That is what copyleft means, and it is the point: what grew out of this
  stays open for the next person.

## Asking for a closed-source exception

The AGPL does not let you keep your source closed. There is no clause to read
carefully here — it is simply not permitted, and no amount of separating
processes or shipping the parts apart changes it.

What is permitted is asking. **Write to rgoshbbb@gmail.com**, say:

- what your project is and where it lives,
- which parts of Pro Engineer you are using,
- whether you are charging for it.

Each request is decided on its own, and asking is not the same as being granted:
what is being asked for is the one thing the licence otherwise refuses. Small,
free and non-commercial work has the best case for it. Nothing is agreed until
it is agreed in writing, naming your project — and a grant covers that project
and no other.

The one answer that is certain in advance is the answer to not asking.

## Commercial licensing

If you want to build a product on this core and sell it, that is welcome, and
it needs a commercial licence: the right to ship the code inside a closed
product, without the AGPL's obligations.

There is no price list, because there is no standard case. What you are
building, and how it is sold, decides what is reasonable — so describe both.
**rgoshbbb@gmail.com.**

What is not on the table is finding out afterwards. A closed product already
shipping with this code inside it was never licensed to exist, and what applies
to it is [further down](#what-happens-if-someone-does-not-ask).

## What counts as using this code

More than copy-and-paste, and this comes up often enough to answer here.

**Covered.** Copied files, copied functions, a modified fork, and a
**translation into another language** — including one an AI wrote. Copyright
protects the expression, not the syntax: the structure, the order of
operations, the decomposition into functions, the names, the comments, the
thresholds and the constants all survive a translation to C++ or Python, and
they are what identifies a copy. This project has unusually distinctive ones —
the `OverlayFrame` layout and the order of its fields, the generated
`frame_layout.lua`, `BRIDGE_PROTOCOL`, the eight advice slots, the specific
verdict thresholds in `engineer.rs`. A machine translation carries every one of
them across.

**Not covered.** Ideas, algorithms and facts. Reading the README, understanding
how a race engineer decides that a tyre is cold, and writing your own
implementation without working from this source infringes nothing, in any
language. That is how copyright works everywhere and it is not something a
licence can change.

## What happens if someone does not ask

Section 8 of the AGPL is automatic: use the code outside the licence and the
licence terminates by itself, with no notice and no court involved. From that
moment there is no permission of any kind — not the closed-source part, all of
it — and copying the work of a copyright holder without permission is
infringement, with the usual consequences: takedown notices to whoever hosts or
sells the product, and a claim for damages.

Reinstatement is possible for a first violation that is cured, but the licence
grants it only once, and only before the copyright holder has given notice.

Separately from the licence, and as a matter of policy: **a project that has
been found using this code outside its licence will not be sold a commercial
one.** Asking first is cheaper than being found.

## The name

The licence covers the code. It grants nothing in the name **Pro Engineer**,
the name **RaceEngineer**, or the project's icons and logo — `NOTICE` states
this under section 7(e). Fork the code freely; ship it under a name of your
own.

## Versions before v0.3.7

Everything published up to and including **v0.3.6** was MIT-licensed, and that
grant cannot be withdrawn: those versions stay MIT for everyone who has them,
permanently. `LICENSE-MIT-HISTORICAL` keeps those terms. The AGPL applies from
the commit that followed `9ba92a3` onward, which is to say to v0.3.7 and every
release after it.

`shm-bridge/` is not affected at all. It is a fork of Damir Jelić's
[shm-bridge](https://github.com/poljar/shm-bridge) and stays MIT-licensed,
today and going forward, under its own `LICENSE`.

## Contributing

Patches are welcome, and they come with one requirement: a `Signed-off-by` line
certifying the [Developer Certificate of Origin](CONTRIBUTING.md), plus
agreement that the maintainer may also license your contribution commercially.
`CONTRIBUTING.md` explains why — briefly, without it the commercial licence
above becomes impossible to offer honestly, and that is the one way this project
could ever pay for itself.
