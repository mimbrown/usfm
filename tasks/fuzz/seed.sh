#!/usr/bin/env bash
# Seed both fuzz corpora from the conformance inputs: the tcdocs submodule and
# the vendored usfm-grammar fixtures. Idempotent: rerun it after
# `git submodule update` and only new or changed files are written.
#
# A seed is named after the test it came from, with `/` replaced by `__`, so a
# finding traces back to a real file. usfm-grammar's carry a `usfm-grammar__`
# prefix (`usfm-grammar__bugfixes__q4.usfm`,
# `usfm-grammar__autofix__slash_in_text.usfm`). Both targets get their own copy
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
FIXTURES=../../tests/fixtures/usfm-grammar

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

targets=(parse_lossy parse_utf8)
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

while IFS= read -r -d '' origin; do
  # tcdocs/tests/<category>/<case>/origin.usfm -> <category>__<case>.usfm
  relative=${origin#"$TCDOCS/tests/"}
  relative=${relative%/origin.usfm}
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
