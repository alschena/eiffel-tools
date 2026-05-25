#!/usr/bin/env python3
"""
Generate static HTML interaction viewers from experiment JSONL results.

Usage:
  ./render_interactions.py                          # reads ./results, writes ./results/html
  ./render_interactions.py --results path/to/results
  ./render_interactions.py --out path/to/output
"""

import argparse
import html
import json
import re
from pathlib import Path

# ---------------------------------------------------------------------------
# Ablation descriptions (mirrors --no-* CLI help text in llm-correct-features)
# ---------------------------------------------------------------------------

ABLATION_PARTS = {
    "task": "Omit 'The following feature does not verify' instruction",
    "mod":  "Omit 'Only modify body/locals' constraint reminder",
    "pre":  "Omit precondition identifier list from prompt context",
    "post": "Omit postcondition identifier list from prompt context",
    "err":  "Omit AutoProof error message from prompt",
    "sig":  "Omit verbatim feature signature from output-format instruction",
    "syn":  "Omit Eiffel syntax reference for contracts and loops",
}


def disabled_parts(tag):
    """Return list of short codes that are disabled for the given ablation tag."""
    if tag == "full":
        return []
    found = re.findall(r"no_([a-z]+)", tag)
    return [p for p in found if p in ABLATION_PARTS]


def ablation_description(tag):
    """Human-readable description of what is omitted in this ablation."""
    parts = disabled_parts(tag)
    if not parts:
        return "All prompt parts enabled"
    return "; ".join(f"{p}: {ABLATION_PARTS[p]}" for p in parts)


def ablation_link(tag, path_to_abl_dir):
    """HTML anchor linking the ablation tag to its help page."""
    href = f"{path_to_abl_dir}{e(tag)}.html"
    return f'<a href="{href}">{e(tag)}</a>'


def render_ablation_page(tag, index_path):
    off = set(disabled_parts(tag))
    title = f"Ablation: {tag}"
    subtitle = "All prompt parts enabled" if not off else f"{len(off)} part(s) omitted"

    rows = []
    for code, desc in ABLATION_PARTS.items():
        enabled = code not in off
        status  = "✓ enabled" if enabled else "✗ omitted"
        cls     = "enabled" if enabled else "disabled"
        rows.append(
            f'<tr class="part-row {cls}">'
            f'<td>{e(status)}</td>'
            f'<td class="part-name">--no-{e(code)}</td>'
            f'<td>{e(desc)}</td>'
            f'</tr>'
        )
    # class_invariant is never ablated
    rows.append(
        f'<tr class="part-row enabled">'
        f'<td>✓ enabled</td>'
        f'<td class="part-name">(class_invariant)</td>'
        f'<td>Class invariant — never omitted</td>'
        f'</tr>'
    )

    return f"""<!doctype html><html lang="en"><head>
<meta charset="utf-8">
<title>{e(title)}</title>
<style>{CSS}</style>
</head><body>
<div class="index-header">
  <h1>{e(title)}</h1>
  <p>{e(subtitle)}</p>
</div>
<a class="back" href="{e(index_path)}">← back to index</a>
<div class="abl-page">
<table>
  <thead><tr><th>Status</th><th>Flag</th><th>Description</th></tr></thead>
  <tbody>{"".join(rows)}</tbody>
</table>
</div>
</body></html>"""


# ---------------------------------------------------------------------------
# CSS
# ---------------------------------------------------------------------------

CSS = """
* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: system-ui, sans-serif; font-size: 14px; color: #2c3e50; background: #f5f6fa; }
a { color: #2980b9; text-decoration: none; }
a:hover { text-decoration: underline; }
h1, h2, h3, h4 { font-weight: 600; }

/* ---------- index ---------- */
.index-header { background: #2c3e50; color: white; padding: 20px 32px; }
.index-header h1 { font-size: 22px; }
.index-header p  { margin-top: 6px; font-size: 13px; opacity: .75; }
.index-body { padding: 24px 32px; }
table { width: 100%; border-collapse: collapse; background: white;
        border-radius: 6px; overflow: hidden; box-shadow: 0 1px 4px #0001; }
th { background: #34495e; color: white; padding: 9px 12px; text-align: left; font-size: 13px;
     cursor: pointer; user-select: none; white-space: nowrap; }
th:hover { background: #4a6278; }
th.sort-asc::after  { content: " ▲"; font-size: 10px; }
th.sort-desc::after { content: " ▼"; font-size: 10px; }
td { padding: 8px 12px; border-bottom: 1px solid #eee; font-size: 13px; vertical-align: middle; }
tr:last-child td { border-bottom: none; }
tr:hover td { background: #f0f4f8; }
.badge { display:inline-block; padding: 2px 8px; border-radius: 3px; font-size: 11px;
         font-weight: 700; color: white; }
.ok   { background: #27ae60; }
.fail { background: #e74c3c; }

/* ---------- feature page ---------- */
.page-header { background: #2c3e50; color: white; padding: 16px 28px; }
.page-header h1 { font-size: 18px; }
.page-header .meta { margin-top: 6px; font-size: 12px; opacity: .8; display: flex; gap: 24px; flex-wrap: wrap; }
.back { display:inline-block; margin: 14px 28px 0; font-size: 13px; }
.page-body { padding: 16px 28px 40px; }
.interaction { border-left: 4px solid #3498db; background: white; border-radius: 0 6px 6px 0;
               padding: 14px 16px; margin: 12px 0; box-shadow: 0 1px 3px #0001; }
.interaction.applied  { border-color: #27ae60; }
.interaction.rejected { border-color: #e74c3c; }
.ix-title { font-size: 14px; font-weight: 700; margin-bottom: 10px; display: flex; align-items: center; gap: 10px; }
.ix-timing { font-size: 11px; color: #888; font-weight: 400; }
.section-label { font-size: 11px; font-weight: 700; text-transform: uppercase;
                 letter-spacing: .5px; color: #888; margin: 10px 0 4px; }
pre { white-space: pre-wrap; font-size: 12px; padding: 10px 12px; border-radius: 4px;
      overflow-y: auto; max-height: 420px; line-height: 1.5; }
.pre-prompt   { background: #e8f4f8; color: #2c3e50; }
.pre-dark     { background: #2d2d2d; color: #f8f8f2; }
.pre-before   { background: #ffe6e6; color: #2c3e50; }
.pre-after    { background: #e6ffe6; color: #2c3e50; }
.pre-error    { background: #fff3cd; color: #2c3e50; max-height: 200px; }
.sg-block { margin: 8px 0; border: 1px solid #e0e0e0; border-radius: 4px; overflow: hidden; }
.sg-header { padding: 6px 10px; font-size: 12px; display: flex; align-items: center; gap: 8px; }
.sg-header.accepted { background: #eafaf1; }
.sg-header.rejected { background: #fdf0ef; }
.sg-meta { font-size: 11px; color: #666; }
.sg-reason { font-size: 11px; color: #c0392b; margin-top: 2px; padding: 0 10px 6px; }

/* ---------- progress bar ---------- */
.progress-section { margin-top: 20px; background: white; border-radius: 6px;
                    padding: 14px 18px; box-shadow: 0 1px 4px #0001; }
.progress-section h2 { font-size: 15px; margin-bottom: 10px; }
.progress-bar-wrap { background: #ecf0f1; border-radius: 4px; height: 14px; overflow: hidden; margin-bottom: 8px; }
.progress-bar-fill { height: 100%; background: #27ae60; transition: width .3s; border-radius: 4px; }
.progress-stats { display: flex; gap: 24px; font-size: 12px; color: #555; flex-wrap: wrap; }
.progress-stats strong { color: #2c3e50; }

/* ---------- ablation legend / ablation page ---------- */
.legend { margin-top: 24px; }
.legend h2 { font-size: 15px; margin-bottom: 10px; }
.legend table { margin-top: 0; }
.abl-tag { font-family: monospace; font-size: 12px; font-weight: 700; }
.abl-disabled { font-size: 11px; color: #c0392b; font-style: italic; }
.abl-desc { color: #555; font-size: 12px; }
.ablation-note { font-size: 12px; color: #666; margin-top: 6px; }
.abl-page { padding: 24px 32px; }
.abl-page h1 { font-size: 20px; margin-bottom: 6px; }
.abl-page .subtitle { font-size: 13px; color: #666; margin-bottom: 20px; }
.part-row.enabled  td:first-child { color: #27ae60; font-weight: 700; }
.part-row.disabled td:first-child { color: #e74c3c; font-weight: 700; }
.part-name { font-family: monospace; font-size: 13px; }
"""

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def e(s):
    return html.escape(str(s))


def badge(success):
    cls = "ok" if success else "fail"
    label = "OK" if success else "FAIL"
    return f'<span class="badge {cls}">{label}</span>'


def fmt_ts(ts):
    from datetime import datetime, timezone
    return datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%Y-%m-%d %H:%M:%S")


def fmt_cost(cost):
    if cost == 0:
        return "$0 (free)"
    return f"${cost:.6f}"


# ---------------------------------------------------------------------------
# Model page
# ---------------------------------------------------------------------------

def render_model_page(model_slug, records, index_path):
    if not records:
        return ""

    total    = len(records)
    n_ok     = sum(1 for r in records if r.get("success"))
    avg_ix   = sum(r.get("llm_interactions", 0) for r in records) / total
    avg_el   = sum(r.get("total_elapsed_time_seconds", 0.0) for r in records) / total
    tot_cost = sum(r.get("total_cost", 0.0) for r in records)
    model_name = records[0].get("model", model_slug)

    # Breakdown: dataset × ablation → (ok, total)
    from collections import defaultdict
    breakdown = defaultdict(lambda: [0, 0])
    for r in records:
        key = (r.get("dataset", ""), r.get("ablation_tag", ""))
        breakdown[key][1] += 1
        if r.get("success"):
            breakdown[key][0] += 1

    datasets   = sorted({k[0] for k in breakdown})
    ablations  = sorted({k[1] for k in breakdown})

    # Header row
    header_cells = "".join(f"<th><a href='../ablations/{e(a)}.html'>{e(a)}</a></th>" for a in ablations)
    # Data rows per dataset
    data_rows = []
    for ds in datasets:
        cells = []
        for ab in ablations:
            ok, tot = breakdown.get((ds, ab), [0, 0])
            if tot == 0:
                cells.append("<td>—</td>")
            else:
                pct = ok / tot * 100
                col = "#27ae60" if ok == tot else ("#e67e22" if ok > 0 else "#e74c3c")
                cells.append(f'<td style="color:{col};font-weight:700">{ok}/{tot} ({pct:.0f}%)</td>')
        data_rows.append(f'<tr><td><strong>{e(ds)}</strong></td>{"".join(cells)}</tr>')

    return f"""<!doctype html><html lang="en"><head>
<meta charset="utf-8">
<title>Model: {e(model_slug)}</title>
<style>{CSS}</style>
</head><body>
<div class="index-header">
  <h1>Model: {e(model_name)}</h1>
  <p>{n_ok}/{total} features verified across all datasets and ablations</p>
</div>
<a class="back" href="{e(index_path)}">← back to index</a>
<div class="abl-page">
  <table style="margin-bottom:20px;width:auto">
    <thead><tr><th>Metric</th><th>Value</th></tr></thead>
    <tbody>
      <tr><td>Features attempted</td><td>{total}</td></tr>
      <tr><td>Verified (success)</td><td>{n_ok} ({n_ok/total*100:.1f}%)</td></tr>
      <tr><td>Avg interactions</td><td>{avg_ix:.2f}</td></tr>
      <tr><td>Avg elapsed</td><td>{avg_el:.1f}s</td></tr>
      <tr><td>Total cost</td><td>{fmt_cost(tot_cost)}</td></tr>
    </tbody>
  </table>
  <h2 style="margin-bottom:10px">Success rate by dataset × ablation</h2>
  <div style="overflow-x:auto">
  <table>
    <thead><tr><th>Dataset</th>{header_cells}</tr></thead>
    <tbody>{"".join(data_rows)}</tbody>
  </table>
  </div>
</div>
</body></html>"""


# ---------------------------------------------------------------------------
# Feature page
# ---------------------------------------------------------------------------

def render_feature_page(rec, index_path, to_root="../../.."):
    class_name   = rec.get("class_name", "?")
    feature_name = rec.get("feature_name", "?")
    model        = rec.get("model", "?")
    success      = rec.get("success", False)
    n_ix         = rec.get("llm_interactions", 0)
    elapsed      = rec.get("total_elapsed_time_seconds", 0.0)
    total_cost   = rec.get("total_cost", 0.0)
    final_status = rec.get("final_status", "")
    dataset      = rec.get("dataset", "")
    ablation     = rec.get("ablation_tag", "")
    jsonl_file   = rec.get("_jsonl_file", "")
    jsonl_line   = rec.get("_jsonl_line", "")

    parts = []
    abl_desc = ablation_description(ablation)
    abl_href = f"{to_root}/ablations/{e(ablation)}.html"
    parts.append(f"""<!doctype html><html lang="en"><head>
<meta charset="utf-8">
<title>{e(class_name)}.{e(feature_name)} — interactions</title>
<style>{CSS}</style>
</head><body>
<div class="page-header">
  <h1>{e(class_name)}.{e(feature_name)}</h1>
  <div class="meta">
    <span>{badge(success)} {e(final_status)}</span>
    <span>model: {e(model)}</span>
    <span>dataset: {e(dataset)}</span>
    <span>ablation: <a href="{abl_href}" style="color:inherit;text-decoration:underline"><strong>{e(ablation)}</strong></a> — {e(abl_desc)}</span>
    <span>interactions: {n_ix}</span>
    <span>elapsed: {elapsed:.1f}s</span>
    <span>cost: {fmt_cost(total_cost)}</span>
    <span title="source record">{e(jsonl_file)}:{e(jsonl_line)}</span>
  </div>
</div>
<a class="back" href="{e(index_path)}">← back to index</a>
<div class="page-body">
""")

    for ix in rec.get("interactions", []):
        ix_num     = ix.get("interaction_number", "?")
        applied    = ix.get("applied", False)
        verif_t    = ix.get("verification_time_seconds", 0.0)
        ai_t       = ix.get("ai_request_time_seconds", 0.0)
        prompt     = ix.get("prompt", "")
        before     = ix.get("before_code", "")
        after      = ix.get("after_code", "")
        err_after  = ix.get("error_message", "")
        suggestions = ix.get("suggestions", [])
        ix_error   = ix.get("error", "")

        status_label = "APPLIED" if applied else "not applied"
        ix_cls = "applied" if applied else "rejected"
        timing = f"verif={verif_t:.2f}s  ai={ai_t:.2f}s"

        parts.append(f"""<div class="interaction {ix_cls}">
  <div class="ix-title">
    Interaction {e(ix_num)} — {e(status_label)}
    <span class="ix-timing">{e(timing)}</span>
  </div>
""")

        if prompt:
            parts.append(f'<div class="section-label">prompt</div>'
                         f'<pre class="pre-prompt">{e(prompt)}</pre>')

        for si, sg in enumerate(suggestions, 1):
            accepted      = sg.get("accepted", False)
            rejection     = sg.get("rejection_reason", "")
            content       = sg.get("content", "")
            finish        = sg.get("finish_reason", "—")
            p_tok         = sg.get("prompt_tokens", 0)
            c_tok         = sg.get("completion_tokens", 0)
            t_tok         = sg.get("total_tokens", 0)
            sg_cost       = sg.get("cost", 0.0)
            sg_model      = sg.get("model", "")
            sg_cls        = "accepted" if accepted else "rejected"
            tag_cls       = "ok" if accepted else "fail"
            tag_lbl       = "ACCEPTED" if accepted else "REJECTED"
            meta = (f"model={e(sg_model)}  finish={e(finish)}  "
                    f"tokens={p_tok}+{c_tok}={t_tok}  cost={fmt_cost(sg_cost)}")
            reason_html = (f'<div class="sg-reason">reason: {e(rejection)}</div>'
                           if rejection else "")
            parts.append(f"""<div class="sg-block">
  <div class="sg-header {sg_cls}">
    <span>suggestion {si}</span>
    <span class="badge {tag_cls}">{tag_lbl}</span>
    <span class="sg-meta">{meta}</span>
  </div>
  {reason_html}
  <pre class="pre-dark">{e(content)}</pre>
</div>
""")

        if ix_error and not applied:
            parts.append(f'<div class="section-label">error</div>'
                         f'<pre class="pre-error">{e(ix_error)}</pre>')

        if before:
            parts.append(f'<div class="section-label">before</div>'
                         f'<pre class="pre-before">{e(before)}</pre>')
        if after:
            parts.append(f'<div class="section-label">after</div>'
                         f'<pre class="pre-after">{e(after)}</pre>')

        if err_after:
            parts.append(f'<div class="section-label">verification result</div>'
                         f'<pre class="pre-error">{e(err_after)}</pre>')

        parts.append('</div>')  # .interaction

    parts.append('</div></body></html>')
    return "".join(parts)


# ---------------------------------------------------------------------------
# Index page
# ---------------------------------------------------------------------------

def render_progress_section(results_dir):
    pf = results_dir / "progress.json"
    if not pf.exists():
        return ""
    try:
        p = json.loads(pf.read_text())
    except Exception:
        return ""

    from datetime import datetime, timezone
    completed = p.get("completed_runs", 0)
    total     = p.get("total_runs", 0)
    last_run  = p.get("last_run", "")
    started   = p.get("started_at", "")
    updated   = p.get("updated_at", "")

    pct = (completed / total * 100) if total else 0
    done = completed >= total

    # ETA calculation using wall-clock elapsed
    eta_str = ""
    elapsed_str = ""
    try:
        t0 = datetime.fromisoformat(started)
        t1 = datetime.fromisoformat(updated)
        elapsed_s = (t1 - t0).total_seconds()
        if elapsed_s > 0:
            mins, secs = divmod(int(elapsed_s), 60)
            hrs, mins  = divmod(mins, 60)
            elapsed_str = f"{hrs}h {mins:02d}m {secs:02d}s" if hrs else f"{mins}m {secs:02d}s"
        if completed > 0 and not done and elapsed_s > 0:
            rate = completed / elapsed_s        # runs per second
            remaining_s = (total - completed) / rate
            m, s = divmod(int(remaining_s), 60)
            h, m = divmod(m, 60)
            eta_str = f"{h}h {m:02d}m {s:02d}s" if h else f"{m}m {s:02d}s"
    except Exception:
        pass

    status = "Complete" if done else "Running"
    bar_color = "#27ae60" if done else "#3498db"
    last_html = f"<span>last completed: <strong>{e(last_run)}</strong></span>" if last_run else ""
    eta_html  = f"<span>ETA: <strong>{e(eta_str)}</strong></span>" if eta_str else ""
    elapsed_html = f"<span>elapsed: <strong>{e(elapsed_str)}</strong></span>" if elapsed_str else ""
    updated_html = f"<span>updated: {e(updated[:19].replace('T', ' '))}</span>" if updated else ""

    return f"""
<div class="progress-section">
  <h2>Experiment progress — {e(status)}</h2>
  <div class="progress-bar-wrap">
    <div class="progress-bar-fill" style="width:{pct:.1f}%;background:{bar_color}"></div>
  </div>
  <div class="progress-stats">
    <span>runs: <strong>{completed} / {total}</strong> ({pct:.1f}%)</span>
    {elapsed_html}
    {eta_html}
    {last_html}
    {updated_html}
  </div>
</div>"""


def render_index(records_with_paths, results_dir=None):
    sorted_records = sorted(records_with_paths,
                            key=lambda r: r[0].get("completed_at", 0), reverse=True)

    rows = []
    for rec, page_path in sorted_records:
        class_name   = rec.get("class_name", "?")
        feature_name = rec.get("feature_name", "?")
        success      = rec.get("success", False)
        n_ix         = rec.get("llm_interactions", 0)
        elapsed      = rec.get("total_elapsed_time_seconds", 0.0)
        total_cost   = rec.get("total_cost", 0.0)
        dataset      = rec.get("dataset", "")
        model_slug   = rec.get("model_slug", "")
        ablation     = rec.get("ablation_tag", "")
        final_status = rec.get("final_status", "")
        ts           = rec.get("completed_at", 0)
        rows.append(
            f'<tr>'
            f'<td data-sort="{ts}">{e(fmt_ts(ts))}</td>'
            f'<td>{e(dataset)}</td>'
            f'<td><a href="models/{e(model_slug)}.html">{e(model_slug)}</a></td>'
            f'<td><a href="ablations/{e(ablation)}.html">{e(ablation)}</a></td>'
            f'<td><a href="{e(page_path)}">{e(class_name)}.{e(feature_name)}</a></td>'
            f'<td data-sort="{1 if success else 0}">{badge(success)}</td>'
            f'<td data-sort="{n_ix}">{n_ix}</td>'
            f'<td data-sort="{elapsed:.3f}">{elapsed:.1f}s</td>'
            f'<td data-sort="{total_cost:.8f}">{fmt_cost(total_cost)}</td>'
            f'<td>{e(final_status)}</td>'
            f'</tr>'
        )

    n_total = len(records_with_paths)
    n_ok    = sum(1 for r, _ in records_with_paths if r.get("success"))

    # Build legend rows: one row per known part
    legend_rows = "".join(
        f'<tr>'
        f'<td class="abl-tag">--no-{code}</td>'
        f'<td class="abl-desc">{e(desc)}</td>'
        f'</tr>'
        for code, desc in ABLATION_PARTS.items()
    )

    sort_js = """
<script>
document.addEventListener('DOMContentLoaded', function() {
  const table = document.querySelector('table');
  const tbody = table.querySelector('tbody');
  const ths   = table.querySelectorAll('thead th');
  let sortCol = 0, sortAsc = false;  // default: Completed descending

  function cellVal(row, col) {
    const td = row.cells[col];
    // numeric: strip non-numeric except dot/minus
    const raw = td.getAttribute('data-sort') || td.innerText.trim();
    const num = parseFloat(raw.replace(/[^0-9.\\-]/g, ''));
    return isNaN(num) ? raw.toLowerCase() : num;
  }

  ths.forEach(function(th, i) {
    th.addEventListener('click', function() {
      if (sortCol === i) { sortAsc = !sortAsc; }
      else { sortCol = i; sortAsc = true; }
      ths.forEach(h => h.classList.remove('sort-asc', 'sort-desc'));
      th.classList.add(sortAsc ? 'sort-asc' : 'sort-desc');
      const rows = Array.from(tbody.rows);
      rows.sort(function(a, b) {
        const va = cellVal(a, i), vb = cellVal(b, i);
        const cmp = va < vb ? -1 : va > vb ? 1 : 0;
        return sortAsc ? cmp : -cmp;
      });
      rows.forEach(r => tbody.appendChild(r));
    });
  });
});
</script>
"""

    progress_html = render_progress_section(results_dir) if results_dir else ""

    return f"""<!doctype html><html lang="en"><head>
<meta charset="utf-8">
<title>Experiment results</title>
<style>{CSS}</style>
</head><body>
<div class="index-header">
  <h1>Experiment results</h1>
  <p>{n_ok}/{n_total} features verified &nbsp;·&nbsp; click a feature to view interactions</p>
</div>
<div class="index-body">
{progress_html}
<table style="margin-top:20px">
  <thead><tr>
    <th class="sort-desc">Completed</th><th>Dataset</th><th>Model</th><th>Ablation</th><th>Feature</th>
    <th>Result</th><th>Interactions</th><th>Elapsed</th><th>Cost</th><th>Status</th>
  </tr></thead>
  <tbody>{"".join(rows)}</tbody>
</table>
<div class="legend">
  <h2>Ablation legend</h2>
  <p class="ablation-note">
    Ablation tag encodes which prompt parts are <em>omitted</em>.
    <code>full</code> = all parts present.
    Tags are concatenated: e.g. <code>no_task_no_err</code> omits task instruction and error message.
    <code>class_invariant</code> is never ablated.
  </p>
  <table style="margin-top:10px">
    <thead><tr><th>Flag</th><th>Description</th></tr></thead>
    <tbody>{legend_rows}</tbody>
  </table>
</div>
</div>
{sort_js}
</body></html>"""


# ---------------------------------------------------------------------------
# Cost extraction (mirrors notebook logic)
# ---------------------------------------------------------------------------

def extract_metrics(interactions):
    total_cost = total_pt = total_ct = total_tt = 0
    for ix in interactions:
        for sg in ix.get("suggestions", []):
            total_cost += sg.get("cost", 0.0)
            total_pt   += sg.get("prompt_tokens", 0)
            total_ct   += sg.get("completion_tokens", 0)
            total_tt   += sg.get("total_tokens", 0)
    return dict(total_cost=total_cost, total_prompt_tokens=total_pt,
                total_completion_tokens=total_ct, total_tokens=total_tt)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--results", default="results",
                   help="Path to results directory (default: ./results)")
    p.add_argument("--out", default=None,
                   help="Output directory (default: <results>/html)")
    args = p.parse_args()

    results_dir = Path(args.results)
    out_dir     = Path(args.out) if args.out else results_dir / "html"
    out_dir.mkdir(parents=True, exist_ok=True)

    records_with_paths = []  # (rec, relative_html_path_from_out_dir)

    for dataset_dir in sorted(results_dir.iterdir()):
        if not dataset_dir.is_dir() or dataset_dir.name == "html":
            continue
        for model_dir in sorted(dataset_dir.iterdir()):
            if not model_dir.is_dir():
                continue
            for jf in sorted(model_dir.glob("*.jsonl")):
                ablation = jf.stem
                with open(jf) as f:
                    for lineno, line in enumerate(f, 1):
                        s = line.strip()
                        if not s or s.startswith(">>"):
                            continue
                        try:
                            rec = json.loads(s)
                        except json.JSONDecodeError:
                            continue

                        rec["dataset"]      = dataset_dir.name
                        rec["model_slug"]   = model_dir.name
                        rec["ablation_tag"] = ablation
                        rec["_jsonl_file"]  = str(jf)
                        rec["_jsonl_line"]  = lineno
                        rec.update(extract_metrics(rec.get("interactions", [])))

                        class_name   = rec.get("class_name", "unknown")
                        feature_name = rec.get("feature_name", "unknown")
                        page_name    = f"{class_name}.{feature_name}.html"
                        page_dir     = out_dir / dataset_dir.name / model_dir.name / ablation
                        page_dir.mkdir(parents=True, exist_ok=True)
                        page_abs  = page_dir / page_name
                        page_rel  = page_abs.relative_to(out_dir)

                        # Relative path back to index.html from the feature page
                        depth   = len(page_rel.parts) - 1
                        to_root = "/".join([".."] * depth)
                        index_rel = f"{to_root}/index.html" if to_root else "index.html"

                        page_html = render_feature_page(rec, index_rel, to_root=to_root)
                        page_abs.write_text(page_html, encoding="utf-8")
                        records_with_paths.append((rec, str(page_rel)))

    # Ablation pages
    all_tags = {rec.get("ablation_tag") for rec, _ in records_with_paths}
    abl_dir  = out_dir / "ablations"
    abl_dir.mkdir(exist_ok=True)
    for tag in all_tags:
        if not tag:
            continue
        abl_html = render_ablation_page(tag, index_path="../index.html")
        (abl_dir / f"{tag}.html").write_text(abl_html, encoding="utf-8")

    # Model pages
    from collections import defaultdict
    by_model = defaultdict(list)
    for rec, _ in records_with_paths:
        by_model[rec.get("model_slug", "")].append(rec)
    models_dir = out_dir / "models"
    models_dir.mkdir(exist_ok=True)
    for model_slug, recs in by_model.items():
        if not model_slug:
            continue
        m_html = render_model_page(model_slug, recs, index_path="../index.html")
        (models_dir / f"{model_slug}.html").write_text(m_html, encoding="utf-8")

    index_html = render_index(records_with_paths, results_dir=results_dir)
    (out_dir / "index.html").write_text(index_html, encoding="utf-8")

    n = len(records_with_paths)
    print(f"Wrote {n} feature page(s) + {len(all_tags)} ablation page(s)"
          f" + {len(by_model)} model page(s) + index → {out_dir}/index.html")


if __name__ == "__main__":
    main()
