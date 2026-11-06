# Board project

A KiCad board project created from [Boardroom](https://github.com/sequoia-hope/boardroom),
designed phase by phase with Claude Code and `.claude/skills/pcb-design/`.

For the full suite (Boardroom, pcbview, UltraRoute, Circuit Composer, Waffle Iron)
and how the files fit together, see [sequoia-hope/boardroom](https://github.com/sequoia-hope/boardroom).

## Check tools

```sh
scripts/doctor.sh
```

If you cloned without `--recurse-submodules`, initialize the pinned tools first:

```sh
git submodule update --init
```

## Serve the project page

`index.html` reads `design/status.json` and embeds the board viewer from `viewer/`.

```sh
./serve.sh --port N
```

Pass an explicit `--port N` or set `PORT=N` (or use `proj port` if registered).

## Rebuild the viewer

Register each board variant under `hw/<board>/<variant>/` in `pcbview.toml`, then run:

```sh
scripts/view.sh build pcbview.toml
scripts/view.sh models pcbview.toml
```

`viewer/` is gitignored by default. To publish on GitHub Pages, remove `viewer/`
from `.gitignore`, add an empty `.nojekyll` file at the root, and commit the build.

## Route

`scripts/route.sh` builds `vendor/ultraroute` into `build/` on first use. Phase 7
runs the router through the skill's `autoroute.py`, a few seeds in turn, each judged
by KiCad's DRC, and records the runs in `hw/<board>/<variant>/route/autoroute.json`
(committed; the per-seed boards and logs beside it are not):

```sh
python3 .claude/skills/pcb-design/scripts/autoroute.py hw/<board>/<variant> --planes In1.Cu,In2.Cu
scripts/route.sh --build        # build the router only
```

## Bump submodules

```sh
git -C vendor/pcbview pull
git add vendor/pcbview && git commit -m "Bump pcbview"

git -C vendor/ultraroute pull
git add vendor/ultraroute && git commit -m "Bump ultraroute"
```

After bumping `vendor/ultraroute`, `scripts/route.sh` rebuilds on the next run;
re-run phase 7 and its gate so the route matches the new router pin.
