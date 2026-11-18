# Security policy

## Supported versions

Security fixes go into the latest release, and into `dev` on the way there. Older releases don't
get fixes: update with `leon self-update`, `brew upgrade`, or the latest package from the
[releases page](https://github.com/aindong/leon-cli/releases/latest).

## Reporting a vulnerability

Please don't open a public issue, pull request or discussion for a security problem. Report it
privately through GitHub instead: on the repository's **Security** tab, choose
**Report a vulnerability** ([direct link](https://github.com/aindong/leon-cli/security/advisories/new)).
Only the maintainers see the report.

Useful things to include:

- what an attacker can do, and what they need first (a local account, access to the tailnet, a
  file they can write, a crafted release or escape sequence…)
- the leon version (`leon --version`), your OS, and whether the app, the server or `leon attach`
  is involved
- the steps to reproduce it, or a proof of concept
- any fix or mitigation you have in mind

## What to expect

- We confirm we got the report within a week.
- We work out the impact with you, and tell you whether we accept it as a vulnerability and how we
  plan to fix it.
- The fix ships in a release, with a GitHub security advisory that credits you, unless you'd
  rather not be named. Please keep the problem private until that release is out.

Areas that matter most: the local socket and its peer checks, the tailnet listener, the update
and its signature checks, and the parsing of terminal output.
