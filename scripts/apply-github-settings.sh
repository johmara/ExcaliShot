#!/usr/bin/env bash
# Applies the recommended GitHub repository settings for johmara/ExcaliShot.
# Safe to re-run: every call sets a state, and rulesets are updated by name.
# Needs: gh (logged in as a repo admin) and jq.
#   ./scripts/apply-github-settings.sh           apply
#   ./scripts/apply-github-settings.sh --dry-run print what would be run
set -uo pipefail

REPO="johmara/ExcaliShot"
DRY=0
[ "${1:-}" = "--dry-run" ] && DRY=1
export GH_PAGER=cat

failed=()

# step "description" gh-args...   (reads an optional JSON body from $BODY)
step() {
  local desc="$1"; shift
  echo "==> $desc"
  if [ "$DRY" = 1 ]; then
    echo "    gh $*"
    [ -n "${BODY:-}" ] && echo "$BODY" | sed 's/^/    /'
    return 0
  fi
  local out rc
  if [ -n "${BODY:-}" ]; then
    out=$(printf '%s' "$BODY" | gh "$@" --input - 2>&1); rc=$?
  else
    out=$(gh "$@" 2>&1); rc=$?
  fi
  if [ $rc -ne 0 ]; then
    echo "    FAILED: $out" | head -5
    failed+=("$desc")
  else
    echo "    ok"
  fi
}

# upsert_ruleset name json
upsert_ruleset() {
  local name="$1" json="$2" id=""
  if [ "$DRY" = 0 ]; then
    id=$(gh api "repos/$REPO/rulesets" --jq ".[] | select(.name==\"$name\") | .id" 2>/dev/null | head -1)
  fi
  BODY="$json"
  if [ -n "$id" ]; then
    step "Update ruleset $name" api -X PUT "repos/$REPO/rulesets/$id"
  else
    step "Create ruleset $name" api -X POST "repos/$REPO/rulesets"
  fi
  BODY=""
}

# --- Repository: merge options, branch cleanup, wiki, description, topics
step "Merge options, auto-delete head branches, wiki off" \
  api -X PATCH "repos/$REPO" \
  -F delete_branch_on_merge=true -F allow_squash_merge=true \
  -F allow_merge_commit=false -F allow_rebase_merge=false -F has_wiki=false

step "Description and topics" \
  repo edit "$REPO" --description "Screenshots with Excalidraw annotation (Tauri, Rust, React)" \
  --add-topic tauri --add-topic screenshot --add-topic excalidraw --add-topic rust --add-topic react

# --- Security features
step "Secret scanning and push protection" \
  api -X PATCH "repos/$REPO" \
  -f 'security_and_analysis[secret_scanning][status]=enabled' \
  -f 'security_and_analysis[secret_scanning_push_protection][status]=enabled'

step "Dependabot alerts" api -X PUT "repos/$REPO/vulnerability-alerts"
step "Dependabot security updates" api -X PUT "repos/$REPO/automated-security-fixes"
step "Private vulnerability reporting" api -X PUT "repos/$REPO/private-vulnerability-reporting"

# --- Actions
step "Default workflow token: read-only, no PR approval by Actions" \
  api -X PUT "repos/$REPO/actions/permissions/workflow" \
  -f default_workflow_permissions=read -F can_approve_pull_request_reviews=false

step "Fork PR approval: all external contributors" \
  api -X PUT "repos/$REPO/actions/permissions/fork-pr-contributor-approval" \
  -f approval_policy=all_external_contributors

step "Allowed actions: selected" \
  api -X PUT "repos/$REPO/actions/permissions" -F enabled=true -f allowed_actions=selected

step "Allowed actions: GitHub-owned plus the three third-party actions" \
  api -X PUT "repos/$REPO/actions/permissions/selected-actions" \
  -F github_owned_allowed=true -F verified_allowed=false \
  -f 'patterns_allowed[]=dtolnay/rust-toolchain@*' \
  -f 'patterns_allowed[]=Swatinem/rust-cache@*' \
  -f 'patterns_allowed[]=tauri-apps/tauri-action@*'

# --- Rulesets (no PR or status-check requirement: the bump job pushes to main with GITHUB_TOKEN)
upsert_ruleset "main-basic" '{
  "name": "main-basic", "target": "branch", "enforcement": "active",
  "conditions": {"ref_name": {"include": ["~DEFAULT_BRANCH"], "exclude": []}},
  "rules": [{"type": "deletion"}, {"type": "non_fast_forward"}]
}'

# Release tags only: the -rc tags are deleted by the cleanup job and must stay out of scope.
upsert_ruleset "release-tags" '{
  "name": "release-tags", "target": "tag", "enforcement": "active",
  "conditions": {"ref_name": {"include": ["refs/tags/v*"], "exclude": ["refs/tags/v*-rc.*"]}},
  "rules": [{"type": "deletion"}, {"type": "non_fast_forward"}, {"type": "update"}]
}'

echo
if [ ${#failed[@]} -gt 0 ]; then
  echo "These steps failed (set them in the web UI, or re-run after fixing):"
  printf '  - %s\n' "${failed[@]}"
  exit 1
fi
echo "All steps done. Not covered here: CodeQL default setup, social preview, deleting a stale 'release' branch."
