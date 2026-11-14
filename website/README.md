# nitrate.renner.dev

The project homepage, served by GitHub Pages from this folder. Plain static files: no build step,
no external fonts or scripts, a strict Content Security Policy. `releases.js` asks the public
GitHub API for the newest published release (beta or stable) to link the setup, the zip and
`SHA256SUMS.txt` directly and to name the version; without it the buttons point at the releases
page and the sentence stays hidden.

- **Screenshots** in `assets/shots/` are real captures of the player, taken muted and offscreen
  with `--snapshot` by `scripts/screenshots.ps1`, which also writes the copies in `docs/images/`
  for the README. The film is Nitrate's own demo footage (`scripts/demo-footage.py`), so the
  pages carry no third-party media. Retake them when the interface changes.
- **Claims:** every number on the page was measured; the note under the numbers names the build
  and the setup. Update both together, and never add a number that wasn't measured.
- **Deploy:** run the **Website Pages** workflow from the Actions tab (manual only). It needs
  GitHub Pages set to "GitHub Actions" in the repository settings and a DNS record for
  `nitrate.renner.dev` (`CNAME`).
