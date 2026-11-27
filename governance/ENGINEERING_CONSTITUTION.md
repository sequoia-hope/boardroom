# Boardroom Engineering Constitution (v1)

These are the rules for engineering work in Boardroom. They are **prescriptive**. They exist so
that an agent can work alone for days on the router and the owner can trust what comes back.

- What the system is and how it is laid out: `docs/one-system-spec.md` (descriptive).
- How to work: this file.
- Session mechanics (building, serving, booking cores): `CLAUDE.md`.

Where they conflict, **this file wins**, until the owner amends it (section 11).

Written 2026-11-21 at the owner's request, after the mistake recorded in section 12. The owner
has not yet read this version line by line; their corrections replace anything here.

---

## 1. Scope

Everything in this repository, and the lab pages and runs made from it. Sections 3 to 7 bind
hardest on the router (`plan`, `wire`, `route`), the checker (`board`) and the bench tools,
because that is where a wrong belief costs most.

---

## 2. Words

- **The designer's board**: the board as its designer routed it (`designer.kicad_pcb` in the
  lab). **Bare**: the same board with the routing taken off. **Ours**: bare, routed by us.
- **A look**: a picture, opened and viewed by the one who will act on it, in this session. Making
  a picture is not a look. Reading a count, a log line or a diagnostic's text is not a look.
- **KiCad's verdict**: KiCad's own DRC on a board, run on both sides of any comparison.
- **Designer-clean**: a board whose designer's finished version passes KiCad's DRC as it stands:
  nothing unconnected and no error at all. Only on these is an open connection plainly the
  router's fault. (Not the looser thing the bench once counted under this name: no error *added*
  to the stripped board's own.)
- **The test set**: the designer-clean boards of the corpus (`bench/tier.py clean`).
- **A converted failure**: a board, connection or error that a change moved from failing to
  passing.

---

## 3. Look

This is a task about shapes on a board. Counts, logs and diagnostics say **where** to look. They
do not say what is there. Work done without looking is worse work, every time.

### L1. No belief about a board without a look

Any statement about what is on a board, ours or a designer's, comes from a picture you viewed
this session. "The designer ran tracks through the gap", "this pad is shut in by its
neighbours", "the pour is cut here": each needs its picture. A statement taken from a count or
from a tool's text is a guess, and is called a guess wherever it is written.

### L2. When a look is required

| When | What to look at |
|---|---|
| **Before asking the owner anything** about a board, a rule or a trade | The very thing the question is about, on the designer's board and on ours. The picture goes with the question. |
| **Before settling on a cause** and before building a fix on it | The place the diagnostic points at, on every layer the nets there use. |
| **Every failure you act on**: a board that got worse, a connection left open, a wire refused, a new KiCad error | That place, ours beside the designer's. How did the designer get through? |
| **Every success you claim**: a board made whole, a count that fell, an error gone | The converted places themselves. See section 4. |
| **Before a commit that changes routed copper** | The boards that moved most, both ways; the look set; our own boards. |
| **When our checker and KiCad disagree** | The item both name, with the rule's geometry drawn (mask, edge, hole, pour). |
| **After every gate run, for no reason at all** | A handful of boards picked at random, whole ones included. Problems nobody is looking for are found only this way. |

### L3. What a look consists of

`bench/look.py` makes this picture for one place on one board; the gallery (section 9) shows
whole boards.

- The designer's board **beside** ours: same place, same scale.
- **Every layer** the nets in question touch. A front-only picture of a two-layer board hides
  the answer half the time. Where the rule is about mask, the board's edge, holes or a pour, that
  layer is drawn too.
- Close enough that a track's width and a via's hole can be seen, and once far enough out to see
  where the place sits on the board.
- One sentence written down afterwards: what you saw. It goes in the commit message, the note or
  the report that rests on it.

### L4. A designer's error count is not a description

"The designer has 132 mask errors" says nothing about what the designer did. Before an error
count on a designer's board is used as evidence, break it down by which items it is between, and
look at one of each kind.

### L5. Economy

Nobody looks at every layer of 631 boards after every change. The economy allowed is in **how
many**, never in **whether**:

- A claim about one board: look at that board.
- A claim about a group with one cause: look at three of them at least, the smallest among them.
- A tier result: the rows above, plus the random handful.
- Zero is never the right number.

### L6. Not looking is reported

If something was concluded without a look (no time, no picture could be made), the report says
so in those words, next to the conclusion.

---

## 4. A success is checked like a failure

A number that improves is a claim, and it is checked before it is believed. For converted
failures, look at how each kind was converted, and reject the change if it was by any of these:

1. **The checker was loosened.** Something our checker now allows that it refused before. This
   is a decision in its own right (C3), never a side effect of a routing fix.
2. **What is counted changed.** A connection no longer listed, a board dropped from the set, an
   error filed under another name.
3. **A connection counted as made that is not.** A wire ending in a piece of pour that reaches
   nothing, two ends near each other and not joined, copper KiCad takes for another net.
4. **Copper no designer would draw.** A track across a part's pads, through a connector's
   keep-clear, a sliver, a via where solder goes, a long detour round the whole board.
5. **A cost moved elsewhere.** Whole by our count and new errors by KiCad's; one board better
   and its neighbours in the tier worse; the same result in several times the time.
6. **Something made for one board.** A constant, a name or a special case that only one board
   can meet.

A change whose gain cannot be told apart from these is not committed.

---

## 5. Who judges

- **C1. KiCad is the judge.** A before-and-after claim carries KiCad's count on both sides, and
  ours beside it. Where they differ, find out why (KiCad's source is public) before trusting
  either.
- **C2. The designers show what is possible, not what is right.** A designer's board proves a
  way exists. It is looked at (L1) before it is quoted. Designers break rules too: what they did
  is a reason to ask, not permission.
- **C3. A change to what the checker allows is the owner's decision** whenever it accepts
  something KiCad reports as an error. It is put to the owner with pictures of the place on a
  designer's board and on ours, and with KiCad's counts before and after. Until answered,
  nothing changes. The owner's standing answer is no: routed boards are to pass DRC, and design
  rules are not relaxed to get a board through. A board outside the test set is never a reason
  to ask.
- **C4. Judge on our own boards over several orders; the test set is the guard.** A change to
  the router is measured on each of our own boards (easy2350, pendant p1-66, the earring,
  earring-v2a, rgb-earring, p2-132-usb6), routed in several shaken orders (`PLAN_JITTER` 1 to
  4), KiCad judging: what is reported is each board's spread before and after. One run of one
  board proves nothing: its count swings by several from the order alone. The test set
  (`bench/tier.py clean`) then says that nothing got worse: how many of its boards pass
  KiCad's DRC outright, and every board that fell. (Owner, 2026-11-26, "yes" to this as
  proposed: the test set had stood at 188 or 189 of 199 through three days of work while what
  was left open was on our own dense boards.)
- **C5. A board that fails DRC as its designer finished it is not tested on.** It proves nothing
  about a router, and its odd cases (pads closer than the rules, mask openings that meet, pads
  the designer left short of spokes) are not worked on.

---

## 6. What the router is for

- **R1. Completion first.** A wish about how good the copper is never costs a connection. Such
  wishes are applied where they are free and are otherwise scored and reported.
- **R2. Hard rules come only from what is written**: net classes, the board's rules, `.kicad_dru`,
  recorded intent. A guess about what a net needs is a preference.
- **R3. One general method.** Nothing is made for a single board. A move meant for one situation
  acts only where that situation is found, and is measured on boards where it is absent.
- **R4. Aim at the designers' picture.** Ours should read as designed: neighbours side by side,
  planes kept whole, short and direct. Compare the picture with the designer's before any count.
- **R5. One router library, two modes; GPL code never.** The old router's code is in this
  repository whole, on this repository's board model, beside the new router's: the new router
  is one mode and the old router another (`boardroom route --mode old`), and their code is
  shared (owner, 2026-11-26: "When I first started the port, I said, no, don't use code from
  the old one. And I am hereby canceling that requirement. So what I want to build now is one
  unified library with different modes, where the current new router is one mode that can be
  run, and the old router is another mode that can be run, and all of the code is shared. And
  then as we iterate on the new router, we will try some experiments where we pull in
  different parts of the old router, and then we'll look at what succeeds and passes my sniff
  test as far as the aesthetics."). The new router goes on making the copper, because its
  boards look as people's do; the old one's parts are tried inside it for what it does not
  do yet, closing the connections it leaves open first. Each such trial is measured like any
  other change (C4, section 7) and its boards are put before the owner, whose
  eye decides whether the look is kept (R4). The old mode gives the same board as its frozen
  binary wherever this repository's checker has not since changed its mind; a change to
  `board` or `kicad` that alters that is looked at. Nothing is copied from GPL projects into
  the MIT crates: `copperroute`, `freerouting` and `kicad-text` stay behind a process
  boundary. What the old router is and does: `docs/old-router-study.md`.

---

## 7. Changing the router

- **F1. Show the cause before fixing it.** A fix is built on a cause that has been seen (section
  3) and can be stated in a sentence. If a fix works and you cannot say why, it is not a fix yet.
- **F2. Every fix has a test that fails without it.** Take the fix out, run the test, see it
  fail; then put it back.
- **F3. Measure the cost beside the win**: boards worse, new KiCad errors, time. Regressions are
  reported as plainly as gains, and are looked at before the commit, not after.
- **F4. A change that helps its target and hurts others is held back**, kept as a patch in the
  lab with its measurements, not tuned until the numbers come right.
- **F5. `main` builds at every commit**, with the workspace lint and tests clean. Commits are
  small enough that one that turns out wrong can be taken out alone.
- **F6. If the diagnosis was wrong, stop and say so.** Do not improvise a second fix on the
  wreck of the first. Record what was learned and go back to F1.

---

## 8. Working alone

The owner gives the shape of the problem and reviews results. The agent finds the solution, and
is expected to carry on for days without being asked.

**Carry on without asking:** finding causes, fixing them, measuring, committing and pushing
finished and measured work, holding back what does not measure cleanly, choosing what to work on
next from the evidence.

**Stop and ask, with pictures (L2), and carry on meanwhile with whatever does not depend on the
answer:**

- anything under C3;
- a change to this file, or to what a tier or the gate counts;
- dropping a goal, or a direction that would take more than a few days to undo;
- anything that writes outside this repository and the lab.

A question to the owner is short, in plain words, says what was looked at, gives the choices and
a recommendation, and links the picture. If writing the question shows the picture was never
made, make it first: the question often goes away.

---

## 9. The owner's review

The owner reviews by looking, so there is always something current to look at.

- **The gallery** (`bench/gallery.py`, pages in the lab) shows every board of the latest run of
  the test set and our own boards: every copper layer, ours beside the designer's, with what is still open
  marked. It is rebuilt after each run of the test set.
- Each rebuild lists at the top what changed since the last: boards that got better, boards that
  got worse, and the boards the agent itself thinks look wrong.
- The owner's comments on a board are instructions about that kind of board, not only that one.
  When the owner corrects something, the rule goes into this file or the project's notes the
  same day.
- 2026-11-22, the owner: "why would we pick boards that failed DRC as our test set? ... I really
  don't wanna start relaxing design rules. I want things to pass DRC for the most part so can we
  make a test set of only boards that actually start as zero DRC and we need to just not have
  these weird cases it's a waste of time." Hence the test set (section 2), C3's standing answer
  and C5.

---

## 10. Reporting

- Plain words. No names of the code's parts where an ordinary word will do.
- Finished work, on several very different boards, not a board that happens to work.
- What was looked at, and what was not.
- What got worse, as prominently as what got better.
- A mistake of the agent's own is stated first, plainly, with what it changes.

---

## 11. Amendments

Only the owner amends this file. An agent may propose a change, in its own commit titled
"Amend Constitution: ...", with the reason, and does not act on it until the owner agrees.
When the owner corrects the agent's way of working, the agent proposes the amendment that would
have prevented it.

---

## 12. Why section 3 exists

On 2026-11-21 the agent asked the owner to decide whether tracks might run through another pad's
solder mask opening "as the designer of the 120-channel test board does". It had read a
diagnostic's text (the gaps between the pads are closed by mask openings) and a count (132 mask
errors on the designer's board), and joined the two into a story. It had never drawn the board.
One picture of the designer's front and back showed the truth: no track runs between the pads;
each pad has a hole in it and its track is on the back; the 132 errors are between the pads and
a pour. There was no decision to make. The router simply lacked a move the designer used.

Had the owner said yes, the checker would have been loosened for nothing (section 4, item 1).

---

## 13. Checklist

Before a commit that changes what the router or the checker does:

- [ ] The cause was seen in a picture, on every layer that matters, beside the designer's board.
- [ ] A test fails without the fix.
- [ ] Our own boards were routed in several orders, before and after; the test set was run
      as the guard; KiCad judged both sides.
- [ ] The boards that got worse were looked at, and are named in the commit message.
- [ ] The converted places were looked at, and none is converted by a means in section 4.
- [ ] Lint and tests are clean.

Before a question to the owner:

- [ ] The thing asked about was looked at, on the designer's board and on ours.
- [ ] The picture is linked where the owner can open it from a phone.
- [ ] The question says what was looked at, the choices, and a recommendation.
