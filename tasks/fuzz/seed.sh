#!/usr/bin/env bash
# Seed the fuzz corpora from the conformance inputs — the tcdocs submodule and
# the vendored usfm-grammar and machine.py fixtures — and from the benchmark
# corpus's real aligned text, usfm-js's Acts (ticket 10). Idempotent: rerun it after
# `git submodule update` and only new or changed files are written.
#
# A seed is named after the test it came from, with `/` replaced by `__`, so a
# finding traces back to a real file. usfm-grammar's carry a `usfm-grammar__`
# prefix (`usfm-grammar__bugfixes__q4.usfm`,
# `usfm-grammar__autofix__slash_in_text.usfm`) and machine.py's a `machine-py__`
# one (`machine-py__Tes__41MATTes.usfm`), and the aligned books a `usfm-js__`
# one (`usfm-js__45-ACT.ult.usfm`). Every target gets its own copy
# (a copy, not a symlink, so a Windows checkout works and so libFuzzer can
# prune one corpus without touching the other).
#
# `--prune` also deletes everything in the corpora that is not a seed, which is
# what libFuzzer added there during a run.
#
# Seeds longer than MAX_LEN are truncated to the last complete line that fits.
# libFuzzer truncates oversized corpus files to `-max_len` anyway; doing it here
# keeps what is committed equal to what is fuzzed, and keeps 5 MB of long
# Bible-sized samples out of git. Their first 64 KiB still exercises the same
# markers.
set -euo pipefail
cd "$(dirname "$0")"

# Matches the `-max_len` in README.md's run commands.
MAX_LEN=${MAX_LEN:-65536}
TCDOCS=../../tcdocs
FIXTURES=../conformance/fixtures/usfm-grammar
MACHINE_PY=../conformance/fixtures/machine-py
ALIGNED=../benchmark/corpus/aligned

if [[ ! -d $TCDOCS/tests ]]; then
  echo "tcdocs is not checked out: run 'git submodule update --init tcdocs'" >&2
  exit 1
fi

prune=false
if [[ ${1:-} == --prune ]]; then
  prune=true
elif [[ $# -gt 0 ]]; then
  echo "usage: $0 [--prune]" >&2
  exit 2
fi

targets=(parse_lossy parse_utf8 parse_html roundtrip)
for target in "${targets[@]}"; do
  mkdir -p "corpus/$target"
done

written=0
truncated=0
seeded=()

# stage <source file> <seed name>
stage() {
  local origin=$1 name=$2 staged
  staged=$(mktemp)
  if [[ $(wc -c <"$origin") -gt $MAX_LEN ]]; then
    head -c "$MAX_LEN" "$origin" | head -n -1 >"$staged"
    truncated=$((truncated + 1))
  else
    cat "$origin" >"$staged"
  fi

  seeded+=("$name")
  for target in "${targets[@]}"; do
    destination="corpus/$target/$name"
    if ! cmp -s "$staged" "$destination"; then
      cp "$staged" "$destination"
      # `mktemp` makes the staging file 0600; a corpus file is ordinary data.
      chmod 644 "$destination"
      written=$((written + 1))
    fi
  done
  rm -f "$staged"
}

# Not seeded: the two tcdocs inputs that quote the New International Version,
# which is Biblica's copyright rather than open data (NOTICE.md). The markup
# they exercise, \vp inside a verse, is in other seeds too.
excluded=(biblica/PublishingVersesNotClosed biblica/PublishingVersesWithFormatting)

while IFS= read -r -d '' origin; do
  # tcdocs/tests/<category>/<case>/origin.usfm -> <category>__<case>.usfm
  relative=${origin#"$TCDOCS/tests/"}
  relative=${relative%/origin.usfm}
  for skip in "${excluded[@]}"; do
    [[ $relative == "$skip" ]] && continue 2
  done
  stage "$origin" "${relative//\//__}.usfm"
done < <(find "$TCDOCS/tests" -name origin.usfm -print0 | sort -z)

# fixtures/usfm-grammar/bugfixes/<case>/origin.usfm
#     -> usfm-grammar__bugfixes__<case>.usfm
while IFS= read -r -d '' origin; do
  relative=${origin#"$FIXTURES/"}
  relative=${relative%/origin.usfm}
  stage "$origin" "usfm-grammar__${relative//\//__}.usfm"
done < <(find "$FIXTURES/bugfixes" -name origin.usfm -print0 | sort -z)

# fixtures/usfm-grammar/autofix/<name>.{usfm,txt}
#     -> usfm-grammar__autofix__<name>.usfm
# The one `.txt` there is USFM too (it opens with `\id TIT`); only the
# extension differs, and the seed carries `.usfm` like every other.
while IFS= read -r -d '' origin; do
  name=$(basename "$origin")
  stage "$origin" "usfm-grammar__autofix__${name%.*}.usfm"
done < <(find "$FIXTURES/autofix" -type f \( -name '*.usfm' -o -name '*.txt' \) -print0 | sort -z)

# fixtures/machine-py/<project>/<book>.SFM -> machine-py__<project>__<book>.usfm
# `Tes/44JHNTes.SFM` is zero bytes on purpose and is staged like the rest;
# `Tes/custom.sty`, the project stylesheet beside the books, is not USFM and is
# not a seed.
while IFS= read -r -d '' origin; do
  relative=${origin#"$MACHINE_PY/"}
  relative=${relative%.SFM}
  stage "$origin" "machine-py__${relative//\//__}.usfm"
done < <(find "$MACHINE_PY" -type f -name '*.SFM' -print0 | sort -z)

# benchmark/corpus/aligned/<book>.usfm -> usfm-js__<book>.usfm
# Whole books, so both are truncated: what is left is most of Acts 1 in the
# ULT (323 alignment groups) and Acts 1–2 in the UGNT (twelve `\k-s` terms).
while IFS= read -r -d '' origin; do
  stage "$origin" "usfm-js__$(basename "$origin")"
done < <(find "$ALIGNED" -type f -name '*.usfm' -print0 | sort -z)

pruned=0
if [[ $prune == true ]]; then
  for target in "${targets[@]}"; do
    while IFS= read -r path; do
      name=$(basename "$path")
      for seed in "${seeded[@]}"; do
        [[ $seed == "$name" ]] && continue 2
      done
      rm -f "$path"
      pruned=$((pruned + 1))
    done < <(find "corpus/$target" -type f)
  done
fi

total=$(find corpus -type f | wc -l)
echo "seeded $total files across ${#targets[@]} corpora ($written written, \
$truncated truncated at $MAX_LEN bytes, $pruned pruned)"
