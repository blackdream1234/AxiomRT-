#!/bin/sh
# AxiomRT v1.0 Industrial Evaluation Kit assembler (AXIOM-KIT-v1.0).
# Requirement reference: Full Completion Mode §19.
#
# Assembles a self-contained evaluation kit under release/ from the
# committed repository: a source tarball (git archive) plus copies of the
# documentation, proofs, evidence, scripts, tests, and the kit documents.
# The output is reproducible from the git tree, so release/ is not
# committed (it is .gitignore'd).
#
# Usage: ./scripts/build_eval_kit.sh
# Output: release/AxiomRT_v1.0_Industrial_Evaluation_Kit/ and
#         release/axiomrt_v1.0_source.tar

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

KIT="release/AxiomRT_v1.0_Industrial_Evaluation_Kit"
rm -rf "$KIT" release/axiomrt_v1.0_source.tar
mkdir -p "$KIT"/source "$KIT"/docs "$KIT"/evidence "$KIT"/proofs \
         "$KIT"/demo "$KIT"/scripts "$KIT"/tests "$KIT"/kit

# Full source as a reproducible tarball from the committed tree.
git archive --format=tar --output=release/axiomrt_v1.0_source.tar HEAD
cp release/axiomrt_v1.0_source.tar "$KIT"/source/

# Human-readable copies of the evaluation-relevant trees.
cp -r docs "$KIT"/docs/
cp -r proofs "$KIT"/proofs/
cp -r evidence "$KIT"/evidence/
cp -r scripts "$KIT"/scripts/
cp -r tests "$KIT"/tests/
cp -r kit "$KIT"/kit/
cp -r examples "$KIT"/demo/ 2>/dev/null || true
cp README.md "$KIT"/README.md

# Top-level kit documents (Full Completion Mode §19).
for d in LIMITATIONS ASSUMPTIONS_OF_USE SAFETY_CONCEPT SECURITY_CONCEPT \
         VERIFICATION_REPORT TEST_REPORT FINAL_REPORT; do
    cp "kit/$d.md" "$KIT/$d.md"
done

echo "Assembled: $KIT"
echo "Source tarball: release/axiomrt_v1.0_source.tar"
