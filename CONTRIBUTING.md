# Contributing

Patches, bug reports and telemetry captures from cars this has never seen are
all welcome. `AGENTS.md` describes the codebase, `CLAUDE.md` how to work in it
without repeating mistakes already made, and `docs/HANDOFF.md` where the current
work stands. Read whichever is relevant before a large change; nothing here is
about the code, only about the paperwork attached to it.

## Sign off your commits

Every commit needs a `Signed-off-by` line:

```bash
git commit -s -m "fix(engineer): the cold-tyre verdict waits for a measured temperature"
```

`-s` adds it from your git identity. The line certifies the Developer
Certificate of Origin, reproduced below: that the code is yours to give.

## And one more grant, which is not standard

By signing off, you also agree that the maintainer may license your
contribution **under the AGPL v3 and under a separate commercial licence**.

This is worth explaining rather than burying, because it asks for something the
DCO alone does not.

Pro Engineer is offered two ways: free under the AGPL for anyone whose own
work is open, and commercially for anyone who wants to build a closed product on
it — see [LICENSING.md](LICENSING.md). The second only works if a single party
holds the rights to relicense the whole thing. If one contribution arrives under
the AGPL alone, the maintainer can no longer offer a commercial licence for the
project without carving that contribution out or removing it — and in practice
that means the contribution gets removed, which serves nobody.

What you get in return is unchanged by any of it: your work stays under the
AGPL, published, with your name on the commit, and you keep your copyright. You
are granting an additional licence, not assigning ownership, and you may do
anything you like with your own code elsewhere.

If you would rather not grant that, say so in the pull request. A patch that
fixes something real is still worth having, and it can usually be reworked or
reimplemented; what cannot happen is it being merged quietly and discovered
later.

## Developer Certificate of Origin 1.1

```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

Everyone is permitted to copy and distribute verbatim copies of this
license document, but changing it is not allowed.


Developer's Certificate of Origin 1.1

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

## Before you open the pull request

The suite has to be green on both targets, and the Lua harnesses take a second
each:

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
```

```bash
luajit apps/lua/tests/run_overlay.lua
```

Commits follow Conventional Commits, in English, with bodies that say what was
wrong, why it mattered and how it was verified. `AGENTS.md` has the examples.
