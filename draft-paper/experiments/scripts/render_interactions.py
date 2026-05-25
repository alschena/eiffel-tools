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
from pathlib import Path

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
th { background: #34495e; color: white; padding: 9px 12px; text-align: left; font-size: 13px; }
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


def fmt_cost(cost):
    if cost == 0:
        return "$0 (free)"
    return f"${cost:.6f}"


# ---------------------------------------------------------------------------
# Feature page
# ---------------------------------------------------------------------------

def render_feature_page(rec, index_path):
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
    <span>ablation: {e(ablation)}</span>
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

def render_index(records_with_paths):
    rows = []
    for rec, page_path in records_with_paths:
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
        rows.append(
            f'<tr>'
            f'<td>{e(dataset)}</td>'
            f'<td>{e(model_slug)}</td>'
            f'<td>{e(ablation)}</td>'
            f'<td><a href="{e(page_path)}">{e(class_name)}.{e(feature_name)}</a></td>'
            f'<td>{badge(success)}</td>'
            f'<td>{n_ix}</td>'
            f'<td>{elapsed:.1f}s</td>'
            f'<td>{fmt_cost(total_cost)}</td>'
            f'<td>{e(final_status)}</td>'
            f'</tr>'
        )

    n_total = len(records_with_paths)
    n_ok    = sum(1 for r, _ in records_with_paths if r.get("success"))

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
<table>
  <thead><tr>
    <th>Dataset</th><th>Model</th><th>Ablation</th><th>Feature</th>
    <th>Result</th><th>Interactions</th><th>Elapsed</th><th>Cost</th><th>Status</th>
  </tr></thead>
  <tbody>{"".join(rows)}</tbody>
</table>
</div>
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

                        page_html = render_feature_page(rec, index_rel)
                        page_abs.write_text(page_html, encoding="utf-8")
                        records_with_paths.append((rec, str(page_rel)))

    index_html = render_index(records_with_paths)
    (out_dir / "index.html").write_text(index_html, encoding="utf-8")

    n = len(records_with_paths)
    print(f"Wrote {n} feature page(s) + index → {out_dir}/index.html")


if __name__ == "__main__":
    main()
