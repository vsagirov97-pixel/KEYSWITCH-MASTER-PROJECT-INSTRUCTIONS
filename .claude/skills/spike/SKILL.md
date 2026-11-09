---
name: spike
description: Start or close a KeySwitch spike (S-1..S-n from docs/04-plan.md) — a time-boxed prototype that removes a technical risk before mass development. Use when the user or plan says to start a spike, or to write a spike's final report.
---
# /spike <ID>

## Start
1. Read the spike row in `docs/04-plan.md` (goal, branch, completion criterion) and related decisions/threats.
2. `git fetch origin main` and create the branch from `origin/main` with the name from the plan.
3. Restate in 3–5 lines: goal, completion criterion, what will be verified where (Linux host / Windows CI / manual), files you will own.
4. Keep the spike minimal: prove or disprove the risk; no polishing, no extra features.

## Close
Write `docs/spikes/<ID>-<slug>.md`:
- Goal and verdict: **confirmed / partially / refuted**.
- What was built (files).
- Evidence: commands run with real output excerpts, CI job names and results.
- Measurements (size, time, memory) with how they were measured.
- Not verified and why.
- Consequences: changes proposed to architecture/decisions (owner approves), follow-up tasks.

Then run `/check`, commit, push the branch, and report to the owner in Russian.
