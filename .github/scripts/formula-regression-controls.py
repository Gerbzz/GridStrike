"""Check pre-fix behavior on an isolated hosted CI checkout, after positive tests.

The new exported length constant stays available so these same regression tests
compile. Only the three behavioral fixes are disabled for the negative controls.
"""

import json
import os
from pathlib import Path
import subprocess


def replace_exact(source, before, after, count=1):
    assert source.count(before) == count, f"Expected {count} matches for {before!r}"
    return source.replace(before, after)


evidence = Path(os.environ["RUNNER_TEMP"]) / "formula-regression-controls"
evidence.mkdir()
head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
parser = Path("crates/formula/src/parser.rs")
source = parser.read_text(encoding="utf-8")
source = replace_exact(
    source,
    '    if body.len() > MAX_FORMULA_LEN && body.chars().count() > MAX_FORMULA_LEN {\n'
    '        return Err(ParseError { msg: format!("formula is longer than {MAX_FORMULA_LEN} characters"), pos: 0 });\n'
    '    }\n',
    "",
)
source = replace_exact(source, "        let mut postfix = 0;\n", "")
source = replace_exact(
    source,
    '                postfix += 1;\n'
    '                if self.depth + postfix > MAX_DEPTH {\n'
    '                    return Err(self.err("formula is nested too deeply"));\n'
    '                }\n',
    "",
    count=2,
)
parser.write_text(source, encoding="utf-8")
stat = Path("crates/functions/src/stat.rs")
stat.write_text(
    replace_exact(
        stat.read_text(encoding="utf-8"),
        "Ok(s.get(n / 2 - 1).copied().unwrap_or(0.0).midpoint(hi))",
        "Ok((s.get(n / 2 - 1).copied().unwrap_or(0.0) + hi) / 2.0)",
    ),
    encoding="utf-8",
)

cargo = ["cargo", "test", "--locked"]
for package in ["formula", "functions", "calc", "xlsx"]:
    cargo.extend(["-p", "gridcraft-" + package])
cargo.append("--lib")
# A compilation error must never count as a successful negative control.
subprocess.run(cargo + ["--no-run"], check=True)
tests = [
    ("tests::formulas_longer_than_excel_allows_are_refused", False),
    ("tests::hostile_formulas_are_errors_not_stack_overflows", False),
    ("tests::malformed::hostile_formula_text_does_not_overflow_the_stack", False),
    ("stat::tests::median_finite_midpoint_does_not_overflow", False),
    ("stat::tests::median_midpoint_keeps_small_values_and_argument_semantics", True),
    ("tests::median_large_values_recalculate_dependents", False),
]
results = []
for test, should_pass in tests:
    result = subprocess.run(
        cargo + [test, "--", "--exact", "--nocapture", "--test-threads=1"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=180,
    )
    (evidence / (test.replace("::", "-") + ".log")).write_text(result.stdout, encoding="utf-8")
    ran = "running 1 test" in result.stdout and test in result.stdout
    expected_failure = any(marker in result.stdout for marker in ["FAILED", "overflowed its stack", "stack overflow"])
    matched = ran and ((result.returncode == 0 and "1 passed" in result.stdout) if should_pass else (result.returncode != 0 and expected_failure))
    results.append({"test": test, "expected_pass": should_pass, "returncode": result.returncode, "matched": matched})
    print(f"{test}: exit {result.returncode}, expected outcome matched={matched}", flush=True)
    print(result.stdout[-3000:], flush=True)

(evidence / "results.json").write_text(json.dumps({"head": head, "behavioral_fixes_disabled": True, "results": results}, indent=2) + "\n", encoding="utf-8")
assert all(result["matched"] for result in results), "A regression control did not produce its expected outcome"
